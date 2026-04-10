use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;
use tracing::{Level, debug, info, span, warn};

use crate::core::definition::WorkflowDefinition;
use crate::core::runtime::{RunEnvironment, WorkflowRunEvent, WorkflowRunSummary};
use crate::error::RunnerError;
use crate::server::{ServerError, WorkflowRegistration, WorkflowServer};

#[derive(Clone)]
pub struct ApiState {
    pub server: Arc<WorkflowServer>,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/workflows", post(upload_workflow))
        .route("/workflows/{workflow_id}", get(get_workflow))
        .route("/workflows/{workflow_id}/runs", post(execute_workflow))
        .route("/runs/{run_id}", get(get_run_summary))
        .route("/runs/{run_id}/resume", post(resume_workflow))
        .route("/runs/{run_id}/events", get(stream_run_events))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let request_id = request
                        .headers()
                        .get("x-request-id")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("");
                    let matched_path = request
                        .extensions()
                        .get::<axum::extract::MatchedPath>()
                        .map(axum::extract::MatchedPath::as_str)
                        .unwrap_or(request.uri().path());

                    span!(
                        Level::INFO,
                        "http_request",
                        method = %request.method(),
                        matched_path = %matched_path,
                        uri = %request.uri(),
                        request_id = %request_id,
                    )
                })
                .on_request(|request: &Request<_>, _span: &tracing::Span| {
                    debug!(method = %request.method(), uri = %request.uri(), "started request");
                })
                .on_response(|response: &Response, latency: Duration, _span: &tracing::Span| {
                    info!(
                        status = response.status().as_u16(),
                        latency_ms = latency.as_millis(),
                        "finished request",
                    );
                })
                .on_failure(|error, latency: Duration, _span: &tracing::Span| {
                    warn!(
                        error = %error,
                        latency_ms = latency.as_millis(),
                        "request failed",
                    );
                }),
        )
        .with_state(state)
}

#[derive(Debug, Deserialize)]
pub struct UploadWorkflowRequest {
    #[serde(rename = "workspaceId", default)]
    pub workspace_id: Option<String>,
    #[serde(rename = "workspaceName", default)]
    pub workspace_name: Option<String>,
    pub workflow: WorkflowDefinition,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteWorkflowRequest {
    #[serde(default)]
    pub trigger: Option<Value>,
    #[serde(default)]
    pub env: Option<RunEnvironment>,
}

#[derive(Debug, Deserialize)]
pub struct ResumeWorkflowRequest {
    pub event: Value,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct WorkflowExecutionAccepted {
    #[serde(rename = "workflowId", skip_serializing_if = "Option::is_none")]
    workflow_id: Option<String>,
    #[serde(rename = "runId")]
    run_id: String,
    status: &'static str,
    #[serde(rename = "statusUrl")]
    status_url: String,
    #[serde(rename = "eventsUrl")]
    events_url: String,
}

async fn health() -> Json<HealthResponse> {
    debug!("health check requested");
    Json(HealthResponse { status: "ok" })
}

async fn upload_workflow(
    State(state): State<ApiState>,
    Json(request): Json<UploadWorkflowRequest>,
) -> Result<Json<WorkflowRegistration>, ApiError> {
    info!(
        workspace_id = request.workspace_id.as_deref().unwrap_or("default"),
        workflow_key = request.workflow.meta.key,
        workflow_version = request.workflow.meta.version,
        "registering workflow",
    );
    let registration = state.server.register_workflow(
        request.workspace_id,
        request.workspace_name,
        request.workflow,
    )?;
    info!(
        workflow_id = %registration.workflow_id,
        workspace_id = %registration.workspace_id,
        workflow_key = %registration.workflow_key,
        workflow_version = registration.workflow_version,
        "workflow registered",
    );
    Ok(Json(registration))
}

async fn get_workflow(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
) -> Result<Json<crate::server::WorkflowRecord>, ApiError> {
    debug!(workflow_id = %workflow_id, "fetching workflow");
    Ok(Json(state.server.get_workflow(&workflow_id)?))
}

async fn execute_workflow(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<ExecuteWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowExecutionAccepted>), ApiError> {
    info!(workflow_id = %workflow_id, "starting workflow run");
    let trigger = request.trigger.unwrap_or_else(default_trigger);
    let env = request.env.unwrap_or_default();
    let summary = state
        .server
        .start_workflow(&workflow_id, trigger, env)
        .await?;
    info!(workflow_id = %workflow_id, run_id = %summary.run_id, "workflow run accepted");

    Ok((
        StatusCode::ACCEPTED,
        Json(WorkflowExecutionAccepted {
            workflow_id: Some(workflow_id),
            run_id: summary.run_id.clone(),
            status: "accepted",
            status_url: format!("/runs/{}", summary.run_id),
            events_url: format!("/runs/{}/events", summary.run_id),
        }),
    ))
}

async fn resume_workflow(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
    Json(request): Json<ResumeWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowExecutionAccepted>), ApiError> {
    info!(run_id = %run_id, "resuming workflow run");
    let summary = state.server.resume_workflow(&run_id, request.event).await?;
    info!(run_id = %summary.run_id, "workflow resume accepted");
    Ok((
        StatusCode::ACCEPTED,
        Json(WorkflowExecutionAccepted {
            workflow_id: None,
            run_id: summary.run_id.clone(),
            status: "accepted",
            status_url: format!("/runs/{}", summary.run_id),
            events_url: format!("/runs/{}/events", summary.run_id),
        }),
    ))
}

async fn get_run_summary(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<Json<WorkflowRunSummary>, ApiError> {
    debug!(run_id = %run_id, "fetching workflow summary");
    let summary = state
        .server
        .get_summary(&run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))?;
    Ok(Json(summary))
}

async fn stream_run_events(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    info!(run_id = %run_id, "subscribing to workflow events");
    let initial_summary = state
        .server
        .get_summary(&run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))?;
    let mut receiver = state.server.subscribe();

    let event_stream = stream! {
        yield Ok(sse_summary_event(&WorkflowRunEvent::from_summary(&initial_summary)));

        loop {
            match receiver.recv().await {
                Ok(event) if event.run_id == run_id => {
                    yield Ok(sse_summary_event(&event));
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(run_id = %run_id, skipped, "event subscriber lagged behind");
                    yield Ok(Event::default()
                        .event("warning")
                        .data(format!("lagged {skipped} run updates")));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::default()))
}

fn sse_summary_event(event: &WorkflowRunEvent) -> Event {
    let data = serde_json::to_string(event).unwrap_or_else(|_| {
        json!({
            "runId": event.run_id,
            "summary": {
                "status": "failed"
            }
        })
        .to_string()
    });

    Event::default().event("summary").data(data)
}

fn default_trigger() -> Value {
    json!({
        "headers": {
            "requestId": "req-demo-1"
        },
        "body": {
            "orderNo": "SO-DEMO-1",
            "bizType": "auto_sort"
        }
    })
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
            Self::Runner(RunnerError::MissingRunSnapshot(message)) => {
                (StatusCode::NOT_FOUND, message)
            }
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
