use std::convert::Infallible;
use std::env;
use std::sync::Arc;
use std::time::Instant;

use async_stream::stream;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{MatchedPath, Path, State};
use axum::http::{HeaderValue, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{debug, info, warn};

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
        .route("/workflows", get(list_workflows).post(upload_workflow))
        .route("/workflows/{workflow_id}", get(get_workflow))
        .route(
            "/workflows/{workflow_id}/runs",
            get(list_workflow_runs).post(execute_workflow),
        )
        .route("/edit-sessions", post(create_edit_session))
        .route(
            "/edit-sessions/{session_id}",
            get(get_edit_session).put(update_edit_session),
        )
        .route(
            "/edit-sessions/{session_id}/ws",
            get(stream_edit_session_ws),
        )
        .route("/runs/{run_id}", get(get_run_summary))
        .route("/runs/{run_id}/resume", post(resume_workflow))
        .route("/runs/{run_id}/terminate", post(terminate_workflow))
        .route("/runs/{run_id}/events", get(stream_run_events))
        .layer(middleware::from_fn(log_http_requests))
        .layer(build_cors_layer())
        .with_state(state)
}

fn build_cors_layer() -> CorsLayer {
    let base_layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers(Any);

    match load_cors_origins() {
        Some(Ok(origins)) => base_layer.allow_origin(AllowOrigin::list(origins)),
        Some(Err(error)) => {
            warn!(
                error = %error,
                "invalid RUNNER_CORS_ALLOW_ORIGINS, falling back to allow-all CORS",
            );
            base_layer.allow_origin(Any)
        }
        None => base_layer.allow_origin(Any),
    }
}

fn load_cors_origins() -> Option<Result<Vec<HeaderValue>, axum::http::header::InvalidHeaderValue>> {
    let raw_origins = env::var("RUNNER_CORS_ALLOW_ORIGINS").ok()?;
    let trimmed = raw_origins.trim();

    if trimmed.is_empty() || trimmed == "*" {
        return None;
    }

    Some(parse_cors_origins(trimmed))
}

fn parse_cors_origins(
    raw_origins: &str,
) -> Result<Vec<HeaderValue>, axum::http::header::InvalidHeaderValue> {
    raw_origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(HeaderValue::from_str)
        .collect()
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
    let should_skip_log = should_skip_access_log(&method, &matched_path);
    let start = Instant::now();

    if !should_skip_log {
        debug!(method = %method, uri = %uri, "started request");
    }

    let response = next.run(request).await;

    if !should_skip_log {
        info!(
            method = %method,
            matched_path = %matched_path,
            uri = %uri,
            request_id = %request_id,
            status = response.status().as_u16(),
            latency_ms = start.elapsed().as_millis(),
            "finished request",
        );
    }

    response
}

fn should_skip_access_log(method: &Method, matched_path: &str) -> bool {
    method == Method::GET && matched_path == "/runs/{run_id}"
}

#[derive(Debug, Deserialize)]
pub struct UploadWorkflowRequest {
    #[serde(rename = "workspaceId", default)]
    pub workspace_id: Option<String>,
    #[serde(rename = "workspaceName", default)]
    pub workspace_name: Option<String>,
    #[serde(rename = "workflowId", default)]
    pub workflow_id: Option<String>,
    #[serde(rename = "editorDocument", default)]
    pub editor_document: Option<Value>,
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
pub struct EditSessionUpsertRequest {
    #[serde(rename = "workspaceId", default)]
    pub workspace_id: Option<String>,
    #[serde(rename = "workflowId", default)]
    pub workflow_id: Option<String>,
    #[serde(rename = "editorDocument", default)]
    pub editor_document: Option<Value>,
    pub workflow: WorkflowDefinition,
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

async fn list_workflows(
    State(state): State<ApiState>,
) -> Result<Json<Vec<crate::store::WorkflowSummaryRecord>>, ApiError> {
    debug!("listing workflows");
    Ok(Json(state.server.list_workflows()?))
}

async fn upload_workflow(
    State(state): State<ApiState>,
    Json(request): Json<UploadWorkflowRequest>,
) -> Result<Json<WorkflowRegistration>, ApiError> {
    info!(
        workspace_id = request.workspace_id.as_deref().unwrap_or("default"),
        workflow_key = request.workflow.meta.key,
        workflow_version = request.workflow.meta.version,
        message = "registering workflow",
    );
    let registration = state.server.register_workflow(
        request.workspace_id,
        request.workspace_name,
        request.workflow_id,
        request.workflow,
        request.editor_document,
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
) -> Result<Json<crate::store::WorkflowDetailRecord>, ApiError> {
    debug!(workflow_id = %workflow_id, "fetching workflow");
    Ok(Json(state.server.get_workflow(&workflow_id)?))
}

async fn create_edit_session(
    State(state): State<ApiState>,
    Json(request): Json<EditSessionUpsertRequest>,
) -> Result<Json<crate::store::WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(state.server.create_edit_session(
        request.workspace_id,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?))
}

async fn get_edit_session(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
) -> Result<Json<crate::store::WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(state.server.get_edit_session(&session_id)?))
}

async fn update_edit_session(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
    Json(request): Json<EditSessionUpsertRequest>,
) -> Result<Json<crate::store::WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(state.server.update_edit_session(
        &session_id,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?))
}

async fn list_workflow_runs(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
) -> Result<Json<Vec<crate::store::WorkflowRunRecord>>, ApiError> {
    debug!(workflow_id = %workflow_id, "listing workflow runs");
    Ok(Json(state.server.list_workflow_runs(&workflow_id)?))
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
    let summary = state
        .server
        .get_summary(&run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))?;
    Ok(Json(summary))
}

async fn terminate_workflow(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<Json<WorkflowRunSummary>, ApiError> {
    info!(run_id = %run_id, "terminating workflow run");
    let summary = state.server.terminate_workflow(&run_id)?;
    Ok(Json(summary))
}

#[cfg(test)]
mod tests {
    use super::{parse_cors_origins, should_skip_access_log};
    use axum::http::Method;

    #[test]
    fn skips_run_summary_polling_access_logs() {
        assert!(should_skip_access_log(&Method::GET, "/runs/{run_id}"));
    }

    #[test]
    fn keeps_other_access_logs_enabled() {
        assert!(!should_skip_access_log(&Method::POST, "/runs/{run_id}"));
        assert!(!should_skip_access_log(
            &Method::GET,
            "/runs/{run_id}/events"
        ));
    }

    #[test]
    fn parses_multiple_cors_origins() {
        let origins = parse_cors_origins("http://localhost:5173, https://ses.example.com")
            .expect("origins should parse");

        let values = origins
            .into_iter()
            .map(|value| value.to_str().expect("header should be utf8").to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            values,
            vec![
                "http://localhost:5173".to_string(),
                "https://ses.example.com".to_string()
            ]
        );
    }
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

async fn stream_edit_session_ws(
    ws: WebSocketUpgrade,
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
) -> Result<Response, ApiError> {
    let initial_session = state.server.get_edit_session(&session_id)?;
    let server = state.server.clone();

    Ok(ws.on_upgrade(move |socket| async move {
        handle_edit_session_socket(socket, server, initial_session).await;
    }))
}

async fn handle_edit_session_socket(
    mut socket: WebSocket,
    server: Arc<WorkflowServer>,
    initial_session: crate::store::WorkflowEditSessionRecord,
) {
    if send_edit_session_message(
        &mut socket,
        &crate::store::WorkflowEditSessionEvent::new("snapshot", initial_session.clone()),
    )
    .await
    .is_err()
    {
        return;
    }

    let mut receiver = server.subscribe_edit_sessions();

    loop {
        tokio::select! {
            received = receiver.recv() => {
                match received {
                    Ok(event) if event.session_id == initial_session.session_id => {
                        if send_edit_session_message(&mut socket, &event).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            session_id = %initial_session.session_id,
                            skipped,
                            "edit session websocket subscriber lagged behind",
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

async fn send_edit_session_message(
    socket: &mut WebSocket,
    event: &crate::store::WorkflowEditSessionEvent,
) -> Result<(), ()> {
    let payload = serde_json::to_string(event).map_err(|_| ())?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
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
