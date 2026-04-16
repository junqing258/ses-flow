use std::env;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing::{debug, info};

use crate::error::RunnerError;
use crate::server::{ServerError, WorkflowServer, edit_session, run, system, workflow};

pub const RUNNER_API_BASE_PATH: &str = "/runner-api";
pub const RUNNER_VIEWS_BASE_PATH: &str = "/views";

#[derive(Clone)]
pub struct ApiState {
    pub server: Arc<WorkflowServer>,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/", get(redirect_to_views))
        .nest_service(RUNNER_VIEWS_BASE_PATH, build_views_service())
        .nest(RUNNER_API_BASE_PATH, build_api_router(state))
}

fn build_api_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(system::health))
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
            put(edit_session::update_edit_session),
        )
        .route("/runs/{run_id}", get(run::get_run_summary))
        .route("/runs/{run_id}/events", get(run::subscribe_run_events))
        .route("/runs/{run_id}/resume", post(run::resume_workflow))
        .route("/runs/{run_id}/terminate", post(run::terminate_workflow))
        .layer(middleware::from_fn(log_http_requests))
        .layer(build_cors_layer())
        .with_state(state)
}

fn build_views_service() -> ServeDir<ServeFile> {
    let static_dir = env::var("RUNNER_STATIC_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/app/views".to_string());

    let index_file = format!("{static_dir}/index.html");
    ServeDir::new(static_dir).fallback(ServeFile::new(index_file))
}

fn build_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
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
    let start = Instant::now();

    debug!(method = %method, uri = %uri, "started request");

    let response = next.run(request).await;

    info!(
        method = %method,
        matched_path = %matched_path,
        uri = %uri,
        request_id = %request_id,
        status = response.status().as_u16(),
        latency_ms = start.elapsed().as_millis(),
        "finished request",
    );

    response
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Runner(RunnerError),
}

impl From<RunnerError> for ApiError {
    fn from(value: RunnerError) -> Self {
        Self::Runner(value)
    }
}

impl From<ServerError> for ApiError {
    fn from(value: ServerError) -> Self {
        match value {
            ServerError::BadRequest(message) => Self::BadRequest(message),
            ServerError::NotFound(message) => Self::NotFound(message),
            ServerError::Runner(error) => Self::Runner(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Runner(RunnerError::MissingRunSnapshot(message)) => (StatusCode::NOT_FOUND, message),
            Self::Runner(RunnerError::Validation(message))
            | Self::Runner(RunnerError::ResumeValidation(message))
            | Self::Runner(RunnerError::Transition(message))
            | Self::Runner(RunnerError::CodeExecution(message))
            | Self::Runner(RunnerError::SubWorkflow(message)) => (StatusCode::BAD_REQUEST, message),
            Self::Runner(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}
