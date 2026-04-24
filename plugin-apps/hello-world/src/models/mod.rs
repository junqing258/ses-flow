use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PLUGIN_ID: &str = "hello_world";
pub const PLUGIN_RUNNER_TYPE: &str = "plugin:hello_world";
pub const FORMAL_PLUGIN_ID: &str = "hello_world_formal";
pub const FORMAL_PLUGIN_RUNNER_TYPE: &str = "plugin:hello_world_formal";

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
    pub input_mapping_schema: Value,
    pub output_mapping_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub plugin_id: String,
    pub version: String,
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
    #[serde(default)]
    pub resume_signal: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CancelRequest {
    pub run_id: String,
    pub request_id: String,
    pub node_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ResumeRequest {
    pub run_id: String,
    pub request_id: String,
    pub node_id: String,
    #[serde(default)]
    pub signal: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResponse {
    pub status: String,
    pub output: Value,
    pub state_patch: Value,
    pub logs: Vec<PluginLogRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLogRecord {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
}
