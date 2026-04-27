use axum::http::HeaderMap;
use axum::response::sse::Event;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub status: String,
    pub transport: String,
    pub timeout_ms: u64,
    pub supports_cancel: bool,
    pub supports_resume: bool,
    pub config_schema: Value,
    pub defaults: Value,
    pub input_schema: Value,
    pub output_schema: Value,
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
#[allow(dead_code)]
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
pub(crate) struct BaseResult<T> {
    pub(crate) code: i32,
    pub(crate) message: String,
    pub(crate) data: T,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub(crate) struct LoginRequest {
    #[serde(alias = "stationId")]
    pub(crate) station_id: String,
    #[serde(default, alias = "platformId")]
    pub(crate) platform_id: Option<String>,
    #[serde(default, alias = "username")]
    pub(crate) username: Option<String>,
    #[serde(default, alias = "password")]
    pub(crate) password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct LoginData {
    pub(crate) authorization: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub(crate) struct ConnectRequest {
    #[serde(default, alias = "clientId")]
    pub(crate) client_id: Option<String>,
    #[serde(default, alias = "stationId")]
    pub(crate) station_id: Option<String>,
    #[serde(default, alias = "platformId")]
    pub(crate) platform_id: Option<String>,
    #[serde(default, alias = "stationIds")]
    pub(crate) station_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ConnectQuery {
    pub(crate) since: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct StationStatusSyncRequest {
    #[serde(default, alias = "stationId")]
    pub(crate) station_id: Option<String>,
    #[serde(default = "default_station_status", alias = "status")]
    pub(crate) status: i32,
    #[serde(default, alias = "platformId")]
    pub(crate) platform_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct StationStatusSyncData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) station_id: Option<String>,
    pub(crate) status: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) platform_id: Option<String>,
}

fn default_station_status() -> i32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct VerifyNotifyRequest {
    #[serde(alias = "sseRequestId", deserialize_with = "string_from_json_value")]
    pub(crate) request_id: String,
    #[serde(default)]
    pub(crate) execution_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct BarcodeRequest {
    #[serde(alias = "barcode")]
    pub(crate) barcode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub(crate) struct TaskInfoRequest {
    #[serde(default, alias = "stationId")]
    pub(crate) station_id: Option<String>,
    #[serde(default, alias = "sku")]
    pub(crate) sku: Option<String>,
    #[serde(default, alias = "barcode")]
    pub(crate) barcode: Option<String>,
    #[serde(default, alias = "completed")]
    pub(crate) completed: Option<i64>,
    #[serde(default, alias = "waveType")]
    pub(crate) wave_type: Option<String>,
    #[serde(default, alias = "lockId")]
    pub(crate) lock_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct TaskInfoResponseData {
    pub(crate) task_id: String,
    pub(crate) chute_id: String,
    pub(crate) wave_id: String,
    pub(crate) order_id: String,
    pub(crate) count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct RobotDepartureRequest {
    #[serde(alias = "taskId")]
    pub(crate) task_id: String,
    #[serde(alias = "agvId")]
    pub(crate) agv_id: String,
    #[serde(alias = "completed")]
    pub(crate) completed: i64,
    #[serde(alias = "requestId")]
    pub(crate) request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct DriveOutRobotRequest {
    pub(crate) agv_id: String,
    pub(crate) station_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct NoBarcodeForceDepartRequest {
    pub(crate) task_id: String,
    pub(crate) agv_id: String,
    pub(crate) request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct FailTaskRequest {
    pub(crate) request_id: String,
    pub(crate) error: TaskErrorPayload,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct SimulateAgvArrivedRequest {
    #[serde(alias = "stationId")]
    pub(crate) station_id: String,
    #[serde(default = "default_simulated_agv_id", alias = "agvId")]
    pub(crate) agv_id: String,
    #[serde(default, alias = "requestId")]
    pub(crate) request_id: Option<u64>,
}

fn default_simulated_agv_id() -> String {
    "AGV-001".to_string()
}

fn string_from_json_value<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(value) => Ok(value),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(serde::de::Error::custom("expected string, number, or bool")),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct TaskErrorPayload {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ExecutionTask {
    pub(crate) execution_id: String,
    pub(crate) run_id: String,
    pub(crate) request_id: String,
    pub(crate) node_id: String,
    pub(crate) trace_id: Option<String>,
    pub(crate) plugin_type: String,
    pub(crate) plugin_id: String,
    pub(crate) target_worker_id: String,
    pub(crate) payload: Value,
    pub(crate) task_id: String,
    pub(crate) wait_signal_type: String,
    pub(crate) state: TaskState,
    pub(crate) runner_base_url: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct TaskSnapshot {
    pub(crate) execution_id: String,
    pub(crate) plugin_type: String,
    pub(crate) task_id: String,
    pub(crate) state: String,
    pub(crate) expires_at: String,
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
pub(crate) enum TaskState {
    Pending,
    Succeeded,
    Failed,
}

impl TaskState {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingEvent {
    pub(crate) event_id: u64,
    pub(crate) request_id: String,
    pub(crate) worker_id: String,
    pub(crate) execution_id: Option<String>,
    pub(crate) message_type: String,
    pub(crate) payload: Value,
    pub(crate) acked_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
}

impl PendingEvent {
    pub(crate) fn to_sse_event(&self) -> Event {
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

pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
}
