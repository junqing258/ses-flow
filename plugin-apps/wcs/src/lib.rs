use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use async_stream::stream;
use axum::body::Body;
use axum::extract::MatchedPath;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Response, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub const DEFAULT_RUNNER_RESUME_SIGNAL: &str = "human_task_done";
pub const HEALTH_PLUGIN_ID: &str = "wcs_bridge";
pub const DEFAULT_CONNECT_WORKER_ID: &str = "anonymous";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub runner_base_url: Option<String>,
    pub heartbeat_interval_secs: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            runner_base_url: std::env::var("RUNNER_BASE_URL").ok().map(normalize_runner_base_url),
            heartbeat_interval_secs: std::env::var("WCS_HEARTBEAT_INTERVAL_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(15),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            runner_base_url: None,
            heartbeat_interval_secs: 15,
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    inner: Arc<RwLock<BridgeState>>,
    event_seq: Arc<AtomicU64>,
    client: Client,
}

impl AppState {
    fn new(config: AppConfig) -> Self {
        Self {
            config,
            inner: Arc::new(RwLock::new(BridgeState::default())),
            event_seq: Arc::new(AtomicU64::new(1)),
            client: Client::new(),
        }
    }

    async fn create_or_get_task(&self, request: ExecuteRequest) -> Result<ExecutionTask, String> {
        let target_worker_id = resolve_worker_id(&request)
            .ok_or_else(|| "missing workerId/stationId/targetWorkerId in config or input".to_string())?;
        let dedupe_key = task_lookup_key(&request.context.run_id, &request.node_id, &request.context.request_id);

        let maybe_existing = {
            let state = self.inner.read().await;
            state
                .task_keys
                .get(&dedupe_key)
                .and_then(|execution_id| state.tasks.get(execution_id))
                .cloned()
        };
        if let Some(task) = maybe_existing {
            return Ok(task);
        }

        let execution_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let runner_base_url = request
            .config
            .get("runnerBaseUrl")
            .and_then(Value::as_str)
            .map(|value| normalize_runner_base_url(value.to_string()))
            .or_else(|| self.config.runner_base_url.clone());
        let signal_type = request
            .config
            .get("waitSignalType")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_RUNNER_RESUME_SIGNAL)
            .to_string();

        let task = ExecutionTask {
            execution_id: execution_id.clone(),
            run_id: request.context.run_id.clone(),
            request_id: request.context.request_id.clone(),
            node_id: request.node_id.clone(),
            trace_id: request.context.trace_id.clone(),
            plugin_type: request.runner_type.clone(),
            plugin_id: request.plugin_id.clone(),
            target_worker_id: target_worker_id.clone(),
            payload: json!({
                "config": request.config,
                "input": request.context.input,
                "env": request.context.env,
            }),
            task_id: request
                .config
                .get("taskId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| execution_id.clone()),
            wait_signal_type: signal_type,
            state: TaskState::Pending,
            runner_base_url,
            created_at: now,
            updated_at: now,
            expires_at: now + Duration::hours(12),
        };

        let event = self
            .queue_pending_event(
                &target_worker_id,
                Some(execution_id.clone()),
                "task.dispatch",
                json!({
                    "ExecutionId": execution_id,
                    "PluginType": task.plugin_type,
                    "PluginId": task.plugin_id,
                    "TaskId": task.task_id,
                    "Payload": task.payload,
                    "ExpiresAt": task.expires_at.to_rfc3339(),
                    "RunId": task.run_id,
                    "NodeId": task.node_id
                }),
            )
            .await;

        {
            let mut state = self.inner.write().await;
            state.task_keys.insert(dedupe_key, task.execution_id.clone());
            state.tasks.insert(task.execution_id.clone(), task.clone());
        }

        info!(
            execution_id = %task.execution_id,
            worker_id = %task.target_worker_id,
            event_id = event.event_id,
            "queued WCS manual task"
        );

        Ok(task)
    }

    async fn queue_pending_event(
        &self,
        worker_id: &str,
        execution_id: Option<String>,
        message_type: &str,
        payload: Value,
    ) -> PendingEvent {
        let event_id = self.event_seq.fetch_add(1, Ordering::SeqCst);
        let request_id = event_id.to_string();

        let (sender, event) = {
            let mut state = self.inner.write().await;
            let sender = state.worker_sender(worker_id);
            let event = PendingEvent {
                event_id,
                request_id,
                worker_id: worker_id.to_string(),
                execution_id,
                message_type: message_type.to_string(),
                payload,
                acked_at: None,
                created_at: Utc::now(),
            };
            state
                .pending_events
                .entry(worker_id.to_string())
                .or_default()
                .push(event.clone());
            (sender, event)
        };
        let _ = sender.send(event.clone());
        event
    }

    async fn login(&self, worker_id: &str) -> String {
        let token = Uuid::new_v4().to_string();
        let mut state = self.inner.write().await;
        state.tokens.insert(token.clone(), worker_id.to_string());
        token
    }

    async fn connect_context(
        &self,
        worker_id: &str,
        since: Option<u64>,
    ) -> (broadcast::Receiver<PendingEvent>, Vec<PendingEvent>, Vec<TaskSnapshot>) {
        let mut state = self.inner.write().await;
        let receiver = state.worker_sender(worker_id).subscribe();
        let backlog = state
            .pending_events
            .get(worker_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.acked_at.is_none())
            .filter(|event| since.is_none_or(|cursor| event.event_id > cursor))
            .collect::<Vec<_>>();
        let snapshots = state
            .tasks
            .values()
            .filter(|task| task.target_worker_id == worker_id && !task.state.is_terminal())
            .map(TaskSnapshot::from)
            .collect::<Vec<_>>();
        (receiver, backlog, snapshots)
    }

    async fn verify_notify(&self, worker_id: &str, request: VerifyNotifyRequest) -> Result<(), String> {
        let mut state = self.inner.write().await;
        let events = state.pending_events.entry(worker_id.to_string()).or_default();
        let maybe_event = events.iter_mut().find(|event| {
            event.request_id == request.request_id
                && request
                    .execution_id
                    .as_ref()
                    .is_none_or(|execution_id| event.execution_id.as_deref() == Some(execution_id.as_str()))
        });
        let event = maybe_event.ok_or_else(|| "pending event not found".to_string())?;
        if event.acked_at.is_none() {
            event.acked_at = Some(Utc::now());
        }
        Ok(())
    }

    async fn current_task_for_worker(&self, worker_id: &str) -> Option<ExecutionTask> {
        let state = self.inner.read().await;
        state
            .tasks
            .values()
            .filter(|task| task.target_worker_id == worker_id && !task.state.is_terminal())
            .max_by_key(|task| task.updated_at)
            .cloned()
    }

    async fn complete_task_with_success(
        &self,
        worker_id: &str,
        request_id: String,
        output: Value,
        state_patch: Value,
        agv_depart_payload: Option<Value>,
    ) -> Result<(), String> {
        let task = self
            .current_task_for_worker(worker_id)
            .await
            .ok_or_else(|| "no active task for worker".to_string())?;
        self.transition_task_success(&task.execution_id, output, state_patch)
            .await?;
        if let Some(payload) = agv_depart_payload {
            self.queue_pending_event(worker_id, Some(task.execution_id), "AGV_DEPART", payload)
                .await;
        }
        info!(worker_id = %worker_id, request_id = %request_id, "worker completed WCS task");
        Ok(())
    }

    async fn fail_task(&self, execution_id: &str, error: TaskErrorPayload) -> Result<(), String> {
        let task = {
            let mut state = self.inner.write().await;
            let task = state
                .tasks
                .get_mut(execution_id)
                .ok_or_else(|| "execution task not found".to_string())?;
            task.state = TaskState::Failed;
            task.updated_at = Utc::now();
            task.clone()
        };

        self.resume_runner(&task, failure_resume_event(&task, &error)).await
    }

    async fn transition_task_success(
        &self,
        execution_id: &str,
        output: Value,
        state_patch: Value,
    ) -> Result<(), String> {
        let task = {
            let mut state = self.inner.write().await;
            let task = state
                .tasks
                .get_mut(execution_id)
                .ok_or_else(|| "execution task not found".to_string())?;
            task.state = TaskState::Succeeded;
            task.updated_at = Utc::now();
            task.clone()
        };

        self.resume_runner(&task, success_resume_event(&task, output, state_patch))
            .await
    }

    async fn cancel_task(&self, request: CancelRequest) -> Result<(), String> {
        let task = {
            let state = self.inner.read().await;
            let execution_id = state
                .task_keys
                .get(&task_lookup_key(&request.run_id, &request.node_id, &request.request_id))
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?;
            state
                .tasks
                .get(&execution_id)
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?
        };
        self.queue_pending_event(
            &task.target_worker_id,
            Some(task.execution_id.clone()),
            "task.cancel",
            json!({
                "ExecutionId": task.execution_id,
                "Reason": request.reason.unwrap_or_else(|| "workflow_terminated".to_string())
            }),
        )
        .await;
        Ok(())
    }

    async fn resume_external(&self, request: ResumeRequest) -> Result<(), String> {
        let task = {
            let state = self.inner.read().await;
            let execution_id = state
                .task_keys
                .get(&task_lookup_key(&request.run_id, &request.node_id, &request.request_id))
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?;
            state
                .tasks
                .get(&execution_id)
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?
        };
        self.queue_pending_event(
            &task.target_worker_id,
            Some(task.execution_id.clone()),
            "task.resume",
            json!({
                "ExecutionId": task.execution_id,
                "Signal": request.signal.unwrap_or(Value::Null)
            }),
        )
        .await;
        Ok(())
    }

    async fn resume_runner(&self, task: &ExecutionTask, event: Value) -> Result<(), String> {
        let Some(base_url) = task.runner_base_url.as_ref() else {
            warn!(execution_id = %task.execution_id, "runner resume skipped because RUNNER_BASE_URL is not configured");
            return Ok(());
        };
        let response = self
            .client
            .post(format!(
                "{}/runs/{}/resume",
                base_url.trim_end_matches('/'),
                task.run_id
            ))
            .json(&json!({ "event": event }))
            .send()
            .await
            .map_err(|error| format!("failed to call runner resume endpoint: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "runner resume endpoint returned {} for execution {}",
                response.status(),
                task.execution_id
            ));
        }
        Ok(())
    }

    async fn health(&self) -> HealthResponse {
        let state = self.inner.read().await;
        let pending_events = state
            .pending_events
            .values()
            .flat_map(|events| events.iter())
            .filter(|event| event.acked_at.is_none())
            .count();
        HealthResponse {
            status: "ok".to_string(),
            plugin_id: HEALTH_PLUGIN_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            online_workers: state.worker_streams.len(),
            active_tasks: state.tasks.values().filter(|task| !task.state.is_terminal()).count(),
            pending_events,
        }
    }
}

#[derive(Default)]
struct BridgeState {
    tasks: HashMap<String, ExecutionTask>,
    task_keys: HashMap<String, String>,
    tokens: HashMap<String, String>,
    worker_streams: HashMap<String, broadcast::Sender<PendingEvent>>,
    pending_events: HashMap<String, Vec<PendingEvent>>,
}

impl BridgeState {
    fn worker_sender(&mut self, worker_id: &str) -> broadcast::Sender<PendingEvent> {
        self.worker_streams
            .entry(worker_id.to_string())
            .or_insert_with(|| broadcast::channel(128).0)
            .clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub id: String,
    pub kind: String,
    pub runner_type: String,
    pub version: String,
    pub category: String,
    pub display_name: String,
    pub description: String,
    pub status: String,
    pub transport: String,
    pub timeout_ms: u64,
    pub supports_cancel: bool,
    pub supports_resume: bool,
    pub config_schema: Value,
    pub defaults: Value,
    pub input_mapping_schema: Value,
    pub output_mapping_schema: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub plugin_id: String,
    pub version: String,
    pub online_workers: usize,
    pub active_tasks: usize,
    pub pending_events: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteRequest {
    pub plugin_id: String,
    pub runner_type: String,
    pub node_id: String,
    #[serde(default)]
    pub config: Value,
    pub context: ExecuteContext,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteContext {
    pub run_id: String,
    pub request_id: String,
    #[serde(default)]
    pub trace_id: Option<String>,
    pub workflow_key: String,
    pub workflow_version: u32,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub env: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub run_id: String,
    pub request_id: String,
    pub node_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeRequest {
    pub run_id: String,
    pub request_id: String,
    pub node_id: String,
    #[serde(default)]
    pub signal: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginResponseEnvelope {
    pub status: String,
    #[serde(default)]
    pub output: Value,
    #[serde(rename = "statePatch", default)]
    pub state_patch: Value,
    #[serde(rename = "waitSignal", skip_serializing_if = "Option::is_none")]
    pub wait_signal: Option<WaitSignal>,
    #[serde(default)]
    pub logs: Vec<PluginLogRecord>,
    #[serde(default)]
    pub error: Option<PluginError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLogRecord {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitSignal {
    #[serde(rename = "type")]
    pub signal_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
struct BaseResult<T> {
    code: i32,
    message: String,
    data: T,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct LoginRequest {
    station_id: String,
    #[serde(default)]
    platform_id: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
struct LoginData {
    authorization: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct ConnectRequest {
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    station_id: Option<String>,
    #[serde(default)]
    platform_id: Option<String>,
    #[serde(default)]
    station_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ConnectQuery {
    since: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct VerifyNotifyRequest {
    request_id: String,
    #[serde(default)]
    execution_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BarcodeRequest {
    barcode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct TaskInfoRequest {
    #[serde(default)]
    station_id: Option<String>,
    #[serde(default)]
    sku: Option<String>,
    #[serde(default)]
    barcode: Option<String>,
    #[serde(default)]
    completed: Option<i64>,
    #[serde(default)]
    wave_type: Option<String>,
    #[serde(default)]
    lock_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
struct TaskInfoResponseData {
    task_id: String,
    chute_id: String,
    wave_id: String,
    order_id: String,
    count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RobotDepartureRequest {
    task_id: String,
    agv_id: String,
    completed: i64,
    request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DriveOutRobotRequest {
    agv_id: String,
    station_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NoBarcodeForceDepartRequest {
    task_id: String,
    agv_id: String,
    request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FailTaskRequest {
    request_id: String,
    error: TaskErrorPayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct TaskErrorPayload {
    code: String,
    message: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ExecutionTask {
    execution_id: String,
    run_id: String,
    request_id: String,
    node_id: String,
    trace_id: Option<String>,
    plugin_type: String,
    plugin_id: String,
    target_worker_id: String,
    payload: Value,
    task_id: String,
    wait_signal_type: String,
    state: TaskState,
    runner_base_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
struct TaskSnapshot {
    execution_id: String,
    plugin_type: String,
    task_id: String,
    state: String,
    expires_at: String,
}

impl From<&ExecutionTask> for TaskSnapshot {
    fn from(task: &ExecutionTask) -> Self {
        Self {
            execution_id: task.execution_id.clone(),
            plugin_type: task.plugin_type.clone(),
            task_id: task.task_id.clone(),
            state: task.state.as_str().to_string(),
            expires_at: task.expires_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TaskState {
    Pending,
    Succeeded,
    Failed,
}

impl TaskState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

#[derive(Debug, Clone)]
struct PendingEvent {
    event_id: u64,
    request_id: String,
    worker_id: String,
    execution_id: Option<String>,
    message_type: String,
    payload: Value,
    acked_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl PendingEvent {
    fn to_sse_event(&self) -> Event {
        Event::default()
            .id(self.event_id.to_string())
            .event(self.message_type.clone())
            .data(
                json!({
                    "MessageType": self.message_type,
                    "EventId": self.event_id,
                    "RequestId": self.request_id,
                    "ExecutionId": self.execution_id,
                    "CreatedAt": self.created_at.to_rfc3339(),
                    "WorkerId": self.worker_id,
                })
                .as_object()
                .cloned()
                .map(|mut envelope| {
                    if let Some(payload) = self.payload.as_object() {
                        envelope.extend(payload.clone());
                    } else {
                        envelope.insert("Data".to_string(), self.payload.clone());
                    }
                    Value::Object(envelope).to_string()
                })
                .unwrap_or_else(|| self.payload.to_string()),
            )
    }
}

pub fn build_app() -> Router {
    build_app_with_config(AppConfig::from_env())
}

pub fn build_app_with_config(config: AppConfig) -> Router {
    let state = AppState::new(config);
    build_router(state)
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/descriptors", get(get_descriptors))
        .route("/descriptor", get(get_descriptor))
        .route("/health", get(get_health))
        .route("/execute", post(execute))
        .route("/cancel", post(cancel))
        .route("/resume", post(resume))
        .route("/station/operation/login", post(login))
        .route("/station/operation/connect", post(connect))
        .route("/station/operation/synchronize", post(synchronize))
        .route("/station/operation/verifyNotify", post(verify_notify))
        .route("/station/operation/scanBarcode", post(scan_barcode))
        .route("/station/operation/getTaskInfo", post(get_task_info))
        .route("/station/operation/robotDeparture", post(robot_departure))
        .route("/station/operation/driveOutRobot", post(drive_out_robot))
        .route("/station/operation/noBarcodeForceDepart", post(no_barcode_force_depart))
        .route("/station/operation/tasks/{execution_id}/fail", post(fail_task))
        .layer(middleware::from_fn(log_http_requests))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state)
}

async fn log_http_requests(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .or_else(|| request.headers().get("requestId").and_then(|value| value.to_str().ok()))
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

async fn get_descriptors() -> Json<Vec<PluginDescriptor>> {
    Json(plugin_descriptors())
}

async fn get_descriptor() -> Json<PluginDescriptor> {
    Json(
        plugin_descriptors()
            .into_iter()
            .next()
            .expect("wcs plugin should expose at least one descriptor"),
    )
}

async fn get_health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(state.health().await)
}

async fn execute(State(state): State<AppState>, Json(request): Json<ExecuteRequest>) -> Response {
    match state.create_or_get_task(request).await {
        Ok(task) => Json(PluginResponseEnvelope {
            status: "waiting".to_string(),
            output: json!({
                "executionId": task.execution_id,
                "workerId": task.target_worker_id,
                "taskId": task.task_id
            }),
            state_patch: json!({
                "wcs": {
                    "executions": {
                        task.execution_id.clone(): {
                            "status": task.state.as_str(),
                            "workerId": task.target_worker_id,
                            "taskId": task.task_id
                        }
                    }
                }
            }),
            wait_signal: Some(WaitSignal {
                signal_type: task.wait_signal_type,
                payload: json!({
                    "executionId": task.execution_id,
                    "requestId": task.request_id
                }),
            }),
            logs: vec![PluginLogRecord {
                level: "info".to_string(),
                message: "manual task dispatched to WCS bridge".to_string(),
                fields: json!({
                    "executionId": task.execution_id,
                    "workerId": task.target_worker_id,
                    "pluginType": task.plugin_type
                }),
            }],
            error: None,
        })
        .into_response(),
        Err(message) => plugin_error(StatusCode::BAD_REQUEST, &message),
    }
}

async fn cancel(State(state): State<AppState>, Json(request): Json<CancelRequest>) -> Response {
    match state.cancel_task(request).await {
        Ok(()) => Json(json!({ "status": "ok" })).into_response(),
        Err(message) => plugin_error(StatusCode::NOT_FOUND, &message),
    }
}

async fn resume(State(state): State<AppState>, Json(request): Json<ResumeRequest>) -> Response {
    match state.resume_external(request).await {
        Ok(()) => Json(json!({ "status": "ok" })).into_response(),
        Err(message) => plugin_error(StatusCode::NOT_FOUND, &message),
    }
}

async fn login(State(state): State<AppState>, Json(request): Json<LoginRequest>) -> Response {
    let token = state.login(&request.station_id).await;
    Json(BaseResult {
        code: 0,
        message: "Success".to_string(),
        data: LoginData {
            authorization: format!("Bearer {token}"),
        },
    })
    .into_response()
}

async fn synchronize() -> Response {
    base_result_ok(Value::Null)
}

async fn connect(
    State(state): State<AppState>,
    Query(query): Query<ConnectQuery>,
    Json(request): Json<ConnectRequest>,
) -> Response {
    let worker_id = worker_id_from_connect(&request);

    let heartbeat_interval_secs = state.config.heartbeat_interval_secs;
    let (mut receiver, backlog, snapshots) = state.connect_context(&worker_id, query.since).await;
    let stream = stream! {
        for event in backlog {
            yield Ok::<Event, Infallible>(event.to_sse_event());
        }
        yield Ok(sync_snapshot_event(&worker_id, snapshots));

        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(heartbeat_interval_secs));
        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(event) => yield Ok(event.to_sse_event()),
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat.tick() => {
                    yield Ok(heartbeat_event(&worker_id));
                }
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(heartbeat_interval_secs)))
        .into_response()
}

async fn verify_notify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<VerifyNotifyRequest>,
) -> Response {
    let worker_id = match worker_id_from_auth(&state, &headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    match state.verify_notify(&worker_id, request).await {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::NOT_FOUND, &message),
    }
}

async fn scan_barcode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BarcodeRequest>,
) -> Response {
    let worker_id = match worker_id_from_auth(&state, &headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = state.current_task_for_worker(&worker_id).await;
    base_result_ok(json!({
        "Barcode": request.barcode,
        "WorkerId": worker_id,
        "TaskId": task.as_ref().map(|item| item.task_id.clone()),
        "ExecutionId": task.as_ref().map(|item| item.execution_id.clone())
    }))
}

async fn get_task_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TaskInfoRequest>,
) -> Response {
    let worker_id = match worker_id_from_auth(&state, &headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = match state.current_task_for_worker(&worker_id).await {
        Some(task) => task,
        None => return base_result_error(StatusCode::NOT_FOUND, "no active task for worker"),
    };
    let data = TaskInfoResponseData {
        task_id: task.task_id,
        chute_id: request
            .sku
            .or(request.barcode)
            .map(|value| format!("C-{}", value.chars().take(3).collect::<String>()))
            .unwrap_or_else(|| "C01".to_string()),
        wave_id: request.wave_type.unwrap_or_else(|| "WAVE-DEMO".to_string()),
        order_id: task.run_id,
        count: request.completed.unwrap_or(0) + 1,
    };
    Json(BaseResult {
        code: 0,
        message: "Success".to_string(),
        data,
    })
    .into_response()
}

async fn robot_departure(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RobotDepartureRequest>,
) -> Response {
    let worker_id = match worker_id_from_auth(&state, &headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let agv_event_payload = json!({
        "AgvId": request.agv_id,
        "StationId": worker_id,
        "TaskId": request.task_id,
        "RequestId": request.request_id
    });
    let state_patch = json!({
        "wcs": {
            "lastRobotDeparture": {
                "taskId": request.task_id,
                "agvId": request.agv_id,
                "completed": request.completed,
                "requestId": request.request_id
            }
        }
    });
    let output = json!({
        "taskId": request.task_id,
        "agvId": request.agv_id,
        "completed": request.completed,
        "requestId": request.request_id
    });
    match state
        .complete_task_with_success(
            &worker_id,
            request.request_id,
            output,
            state_patch,
            Some(agv_event_payload),
        )
        .await
    {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
    }
}

async fn drive_out_robot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DriveOutRobotRequest>,
) -> Response {
    match worker_id_from_auth(&state, &headers).await {
        Ok(_) => base_result_ok(json!({
            "AgvId": request.agv_id,
            "StationId": request.station_id,
            "Forced": true
        })),
        Err(message) => base_result_error(StatusCode::UNAUTHORIZED, &message),
    }
}

async fn no_barcode_force_depart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NoBarcodeForceDepartRequest>,
) -> Response {
    let worker_id = match worker_id_from_auth(&state, &headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let output = json!({
        "taskId": request.task_id,
        "agvId": request.agv_id,
        "requestId": request.request_id,
        "barcodeMissing": true
    });
    let state_patch = json!({
        "wcs": {
            "lastRobotDeparture": {
                "taskId": request.task_id,
                "agvId": request.agv_id,
                "requestId": request.request_id,
                "barcodeMissing": true
            }
        }
    });
    match state
        .complete_task_with_success(&worker_id, request.request_id, output, state_patch, None)
        .await
    {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
    }
}

async fn fail_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
    Json(request): Json<FailTaskRequest>,
) -> Response {
    if let Err(message) = worker_id_from_auth(&state, &headers).await {
        return base_result_error(StatusCode::UNAUTHORIZED, &message);
    }
    match state.fail_task(&execution_id, request.error).await {
        Ok(()) => {
            info!(execution_id = %execution_id, request_id = %request.request_id, "worker reported WCS task failure");
            base_result_ok(Value::Null)
        }
        Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
    }
}

fn plugin_descriptors() -> Vec<PluginDescriptor> {
    vec![
        build_manual_descriptor(
            "manual_pick",
            "plugin:manual_pick",
            "人工拣货",
            "工作站人工拣货桥接节点",
        ),
        build_manual_descriptor(
            "manual_weigh",
            "plugin:manual_weigh",
            "人工称重",
            "工作站人工称重桥接节点",
        ),
    ]
}

fn build_manual_descriptor(id: &str, runner_type: &str, display_name: &str, description: &str) -> PluginDescriptor {
    PluginDescriptor {
        id: id.to_string(),
        kind: "effect".to_string(),
        runner_type: runner_type.to_string(),
        version: "1.0.0".to_string(),
        category: "人工工作台".to_string(),
        display_name: display_name.to_string(),
        description: description.to_string(),
        status: "stable".to_string(),
        transport: "http".to_string(),
        timeout_ms: 0,
        supports_cancel: true,
        supports_resume: true,
        config_schema: json!({
            "type": "object",
            "required": ["workerId"],
            "properties": {
                "workerId": { "type": "string", "title": "工位/工人 ID" },
                "taskId": { "type": "string", "title": "任务 ID" },
                "runnerBaseUrl": { "type": "string", "title": "Runner API Base URL" },
                "waitSignalType": { "type": "string", "title": "恢复信号类型", "default": DEFAULT_RUNNER_RESUME_SIGNAL }
            }
        }),
        defaults: json!({
            "waitSignalType": DEFAULT_RUNNER_RESUME_SIGNAL
        }),
        input_mapping_schema: json!({
            "type": "object"
        }),
        output_mapping_schema: json!({
            "type": "object"
        }),
    }
}

fn resolve_worker_id(request: &ExecuteRequest) -> Option<String> {
    value_string(&request.config, &["workerId", "stationId", "targetWorkerId"])
        .or_else(|| value_string(&request.context.input, &["workerId", "stationId", "targetWorkerId"]))
}

async fn worker_id_from_auth(state: &AppState, headers: &HeaderMap) -> Result<String, String> {
    let token = bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
    let state = state.inner.read().await;
    state
        .tokens
        .get(&token)
        .cloned()
        .ok_or_else(|| "invalid bearer token".to_string())
}

fn worker_id_from_connect(request: &ConnectRequest) -> String {
    request
        .client_id
        .clone()
        .or_else(|| request.station_id.clone())
        .or_else(|| request.station_ids.first().cloned())
        .unwrap_or_else(|| DEFAULT_CONNECT_WORKER_ID.to_string())
}

fn sync_snapshot_event(worker_id: &str, tasks: Vec<TaskSnapshot>) -> Event {
    Event::default().event("sync.snapshot").data(
        json!({
            "MessageType": "sync.snapshot",
            "WorkerId": worker_id,
            "Tasks": tasks
        })
        .to_string(),
    )
}

fn heartbeat_event(worker_id: &str) -> Event {
    Event::default().event("Heart_Beat").data(
        json!({
            "MessageType": "Heart_Beat",
            "WorkerId": worker_id,
            "RcsStatus": "connected",
            "StationList": [worker_id],
            "Ts": Utc::now().to_rfc3339()
        })
        .to_string(),
    )
}

fn task_lookup_key(run_id: &str, node_id: &str, request_id: &str) -> String {
    format!("{run_id}:{node_id}:{request_id}")
}

fn success_resume_event(task: &ExecutionTask, output: Value, state_patch: Value) -> Value {
    json!({
        "type": task.wait_signal_type,
        "payload": {
            "status": "success",
            "output": output,
            "statePatch": state_patch
        }
    })
}

fn failure_resume_event(task: &ExecutionTask, error: &TaskErrorPayload) -> Value {
    json!({
        "type": task.wait_signal_type,
        "payload": {
            "status": "failed",
            "error": {
                "code": error.code,
                "message": error.message,
                "retryable": false
            }
        }
    })
}

fn normalize_runner_base_url(base_url: String) -> String {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.ends_with("/runner-api") {
        trimmed
    } else {
        format!("{trimmed}/runner-api")
    }
}

fn value_string(value: &Value, candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|key| {
        value.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    })
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
}

fn plugin_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn base_result_ok(data: Value) -> Response {
    Json(BaseResult {
        code: 0,
        message: "Success".to_string(),
        data,
    })
    .into_response()
}

fn base_result_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(BaseResult {
            code: status.as_u16() as i32,
            message: message.to_string(),
            data: Value::Null,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
