use std::env;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get, post, put};
use axum::{Json, Router};
use runner::app::{AppError, WorkflowApp};
use runner::error::RunnerError;
use serde::Serialize;
use ses_flow_telemetry::set_span_parent_from_headers;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::{Instrument, debug, field, info, info_span, warn};

use crate::modules::system::system_store::SystemSettingsStore;
use crate::modules::{ai_gateway, edit_session, node_registry, run, system, workflow};

pub const RUNNER_API_BASE_PATH: &str = "/runner-api";
pub const RUNNER_VIEWS_BASE_PATH: &str = "/views";

#[derive(Clone)]
pub struct ApiState {
    pub app: Arc<WorkflowApp>,
    pub ai_gateway_base_url: String,
    pub ai_gateway_client: reqwest::Client,
    pub system_settings: Arc<dyn SystemSettingsStore>,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/", get(redirect_to_views))
        .nest_service(RUNNER_VIEWS_BASE_PATH, build_views_service())
        .nest("/api/ai", build_ai_gateway_router(state.clone()))
        .nest(RUNNER_API_BASE_PATH, build_api_router(state))
}

fn build_api_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(system::health))
        .route(
            "/system/plugin-auto-registration",
            get(system::get_plugin_auto_registration).put(system::update_plugin_auto_registration),
        )
        .route("/node-descriptors", get(node_registry::list_node_descriptors))
        .route(
            "/node-descriptors/{descriptor_id}/versions",
            get(node_registry::get_node_descriptor_versions),
        )
        .route("/plugin-registrations", post(node_registry::register_http_plugin))
        .route("/catalog/refresh", get(workflow::refresh_catalog))
        .route("/workflows/events", get(workflow::subscribe_workflows_events))
        .route(
            "/workflows",
            get(workflow::list_workflows).post(workflow::upload_workflow),
        )
        .route("/workflows/{workflow_id}", get(workflow::get_workflow))
        .route(
            "/workflows/{workflow_id}/events",
            get(workflow::subscribe_workflow_events),
        )
        .route("/workflows/{workflow_id}/runs", get(workflow::list_workflow_runs))
        .route("/workflows/{workflow_id}/run", post(run::execute_workflow))
        .route("/edit-sessions", post(edit_session::create_edit_session))
        .route("/edit-sessions/{session_id}", get(edit_session::get_edit_session))
        .route(
            "/edit-sessions/{session_id}/events",
            get(edit_session::subscribe_edit_session_events),
        )
        .route(
            "/edit-sessions/{session_id}/draft",
            put(edit_session::update_edit_session).patch(edit_session::patch_edit_session),
        )
        .route("/runs/search", get(run::search_runs))
        .route("/runs/{run_id}", get(run::get_run_summary))
        .route("/runs/{run_id}/events", get(run::subscribe_run_events))
        .route("/runs/{run_id}/manual-patch", post(run::manual_patch_run))
        .route("/runs/{run_id}/resume", post(run::resume_workflow))
        .route("/runs/{run_id}/terminate", post(run::terminate_workflow))
        .layer(middleware::from_fn(log_http_requests))
        .layer(build_cors_layer())
        .with_state(state)
}

fn build_ai_gateway_router(state: ApiState) -> Router {
    Router::new()
        .route("/", any(ai_gateway::proxy_root))
        .route("/{*path}", any(ai_gateway::proxy_path))
        .layer(middleware::from_fn(log_http_requests))
        .layer(build_cors_layer())
        .with_state(state)
}

fn build_views_service() -> ServeDir<ServeFile> {
    let static_dir = env::var("BACKEND_STATIC_DIR")
        .ok()
        .or_else(|| env::var("RUNNER_STATIC_DIR").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/app/views".to_string());

    let index_file = format!("{static_dir}/index.html");
    ServeDir::new(static_dir).fallback(ServeFile::new(index_file))
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any)
}

async fn redirect_to_views() -> Redirect {
    Redirect::permanent("/views/")
}

async fn log_http_requests(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or(uri.path())
        .to_string();
    let route_name = format!("{method} {matched_path}");
    let request_span = info_span!(
        "http.request",
        "otel.name" = %route_name,
        "otel.kind" = "server",
        "otel.status_code" = field::Empty,
        "otel.status_description" = field::Empty,
        "http.request.method" = %method,
        "http.route" = %matched_path,
        "url.path" = %uri.path(),
        "url.query" = uri.query().unwrap_or(""),
        "http.response.status_code" = field::Empty,
        "request.id" = %request_id,
        "latency_ms" = field::Empty,
    );
    set_span_parent_from_headers(&request_span, request.headers());

    async move {
        let start = Instant::now();

        debug!(method = %method, uri = %uri, "started request");

        let response = next.run(request).await;

        let status = response.status();
        let latency_ms = start.elapsed().as_millis();
        let current_span = tracing::Span::current();
        current_span.record("http.response.status_code", status.as_u16());
        current_span.record("latency_ms", latency_ms as u64);
        if status.is_server_error() {
            current_span.record("otel.status_code", "error");
            current_span.record("otel.status_description", format!("HTTP {}", status.as_u16()));
        }

        if status.is_client_error() {
            warn!(
                method = %method,
                matched_path = %matched_path,
                uri = %uri,
                request_id = %request_id,
                status = status.as_u16(),
                latency_ms,
                "finished request",
            );
        } else {
            info!(
                method = %method,
                matched_path = %matched_path,
                uri = %uri,
                request_id = %request_id,
                status = status.as_u16(),
                latency_ms,
                "finished request",
            );
        }

        response
    }
    .instrument(request_span)
    .await
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Throttled(String),
    ServiceUnavailable(String),
    Runner(RunnerError),
}

impl From<RunnerError> for ApiError {
    fn from(value: RunnerError) -> Self {
        Self::Runner(value)
    }
}

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        match value {
            AppError::BadRequest(message) => Self::BadRequest(message),
            AppError::NotFound(message) => Self::NotFound(message),
            AppError::Throttled(message) => Self::Throttled(message),
            AppError::QueueTimeout(message) => Self::ServiceUnavailable(message),
            AppError::Runner(error) => Self::Runner(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Throttled(message) => (StatusCode::TOO_MANY_REQUESTS, message),
            Self::ServiceUnavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
            Self::Runner(RunnerError::MissingRunSnapshot(message)) => (StatusCode::NOT_FOUND, message),
            Self::Runner(RunnerError::Validation(message))
            | Self::Runner(RunnerError::ResumeValidation(message))
            | Self::Runner(RunnerError::Transition(message))
            | Self::Runner(RunnerError::CodeExecution(message))
            | Self::Runner(RunnerError::SubWorkflow(message))
            | Self::Runner(RunnerError::PluginRegistration(message)) => (StatusCode::BAD_REQUEST, message),
            Self::Runner(RunnerError::PluginExecution(message)) => (StatusCode::BAD_GATEWAY, message),
            Self::Runner(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}
