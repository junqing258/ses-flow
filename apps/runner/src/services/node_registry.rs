use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::{Builder, Handle};

use crate::core::runtime::{NextSignal, NodeExecutionError, NodeLogRecord, RunEnvironment};
use crate::error::RunnerError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeDescriptorStatus {
    Stable,
    Beta,
    Deprecated,
}

impl Default for NodeDescriptorStatus {
    fn default() -> Self {
        Self::Stable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTransport {
    Builtin,
    Http,
    Grpc,
    Process,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDescriptor {
    pub id: String,
    pub kind: String,
    pub runner_type: String,
    pub version: String,
    pub category: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub status: NodeDescriptorStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<NodeTransport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_app_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    #[serde(rename = "timeoutMs", default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(rename = "supportsCancel", default)]
    pub supports_cancel: bool,
    #[serde(rename = "supportsResume", default)]
    pub supports_resume: bool,
    #[serde(rename = "configSchema", default)]
    pub config_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<Value>,
    #[serde(rename = "inputMappingSchema", default, skip_serializing_if = "Option::is_none")]
    pub input_mapping_schema: Option<Value>,
    #[serde(rename = "outputMappingSchema", default, skip_serializing_if = "Option::is_none")]
    pub output_mapping_schema: Option<Value>,
}

impl NodeDescriptor {
    pub fn validate_http_plugin(&self) -> Result<(), RunnerError> {
        if !self.runner_type.starts_with("plugin:") {
            return Err(RunnerError::PluginRegistration(format!(
                "plugin runnerType must start with plugin:, got {}",
                self.runner_type
            )));
        }

        if !matches!(self.transport, Some(NodeTransport::Http)) {
            return Err(RunnerError::PluginRegistration(format!(
                "plugin {} only supports transport=http in this phase",
                self.id
            )));
        }

        if self.display_name.trim().is_empty() {
            return Err(RunnerError::PluginRegistration(
                "plugin displayName cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeDescriptorRegistry {
    descriptors_by_id: HashMap<String, NodeDescriptor>,
    runner_type_index: HashMap<String, String>,
}

impl NodeDescriptorRegistry {
    pub fn register(&mut self, descriptor: NodeDescriptor) {
        let id = descriptor.id.clone();
        self.runner_type_index
            .insert(descriptor.runner_type.clone(), id.clone());
        self.descriptors_by_id.insert(id, descriptor);
    }

    pub fn unregister_by_endpoints(&mut self, endpoints: &[String]) {
        if endpoints.is_empty() {
            return;
        }

        let endpoint_set = endpoints.iter().collect::<std::collections::HashSet<_>>();
        let removed_runner_types = self
            .descriptors_by_id
            .values()
            .filter(|descriptor| {
                descriptor
                    .endpoint
                    .as_ref()
                    .is_some_and(|endpoint| endpoint_set.contains(endpoint))
            })
            .map(|descriptor| descriptor.runner_type.clone())
            .collect::<Vec<_>>();

        self.descriptors_by_id.retain(|_, descriptor| {
            descriptor
                .endpoint
                .as_ref()
                .is_none_or(|endpoint| !endpoint_set.contains(endpoint))
        });

        self.runner_type_index
            .retain(|runner_type, _| !removed_runner_types.iter().any(|value| value == runner_type));
    }

    pub fn resolve(&self, id: &str) -> Option<NodeDescriptor> {
        self.descriptors_by_id.get(id).cloned()
    }

    pub fn resolve_by_runner_type(&self, runner_type: &str) -> Option<NodeDescriptor> {
        self.runner_type_index
            .get(runner_type)
            .and_then(|id| self.descriptors_by_id.get(id))
            .cloned()
    }

    pub fn list(&self) -> Vec<NodeDescriptor> {
        let mut descriptors = self.descriptors_by_id.values().cloned().collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.id.cmp(&right.id));
        descriptors
    }

    pub fn versions(&self, id: &str) -> Vec<NodeDescriptor> {
        self.resolve(id).into_iter().collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpPluginExecutionRequest {
    pub plugin_id: String,
    pub runner_type: String,
    pub node_id: String,
    pub config: Value,
    pub context: HttpPluginExecutionContext,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpPluginExecutionContext {
    pub run_id: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    pub workflow_key: String,
    pub workflow_version: u32,
    pub input: Value,
    pub state: Value,
    pub env: RunEnvironment,
}

#[derive(Debug, Clone)]
pub struct RegisteredHttpPluginDescriptor {
    pub descriptor: NodeDescriptor,
    pub base_url: String,
}

impl RegisteredHttpPluginDescriptor {
    pub fn new(mut descriptor: NodeDescriptor, base_url: impl Into<String>) -> Result<Self, RunnerError> {
        descriptor.validate_http_plugin()?;
        let base_url = normalize_base_url(base_url.into());
        descriptor.endpoint = Some(base_url.clone());
        Ok(Self { descriptor, base_url })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginResponseEnvelope {
    pub status: String,
    #[serde(default)]
    pub output: Value,
    #[serde(rename = "statePatch", default)]
    pub state_patch: Value,
    #[serde(rename = "waitSignal", alias = "nextSignal", default)]
    pub wait_signal: Option<NextSignal>,
    #[serde(default)]
    pub logs: Vec<PluginLogRecord>,
    #[serde(default)]
    pub error: Option<NodeExecutionError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginLogRecord {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: Value,
}

pub fn extract_request_id_from_value(value: &Value) -> Option<String> {
    find_string_case_insensitive(value, &["requestId", "x-request-id"])
}

pub fn extract_trace_id_from_value(value: &Value) -> Option<String> {
    find_string_case_insensitive(value, &["traceId", "x-trace-id"])
}

pub fn inject_plugin_log_metadata(
    logs: Vec<PluginLogRecord>,
    run_id: &str,
    request_id: &str,
    node_id: &str,
    trace_id: Option<&str>,
    timestamp: DateTime<Utc>,
) -> Vec<NodeLogRecord> {
    logs.into_iter()
        .map(|record| NodeLogRecord {
            level: record.level,
            message: record.message,
            fields: record.fields,
            run_id: Some(run_id.to_string()),
            request_id: Some(request_id.to_string()),
            node_id: Some(node_id.to_string()),
            trace_id: trace_id.map(str::to_string),
            timestamp: Some(timestamp),
        })
        .collect()
}

pub fn normalize_base_url(base_url: String) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

pub fn build_plugin_headers(trace_id: Option<&str>) -> Result<HeaderMap, RunnerError> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(trace_id) = trace_id {
        let value = HeaderValue::from_str(trace_id)
            .map_err(|error| RunnerError::PluginExecution(format!("invalid trace id header value: {error}")))?;
        headers.insert("X-Trace-Id", value);
    }
    Ok(headers)
}

pub fn build_http_client(timeout_ms: Option<u64>) -> Result<reqwest::Client, RunnerError> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }

    builder
        .build()
        .map_err(|error| RunnerError::PluginExecution(error.to_string()))
}

pub fn block_on<F, T>(future: F) -> Result<T, RunnerError>
where
    F: std::future::Future<Output = Result<T, RunnerError>>,
{
    match Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| RunnerError::PluginExecution(error.to_string()))?
            .block_on(future),
    }
}

fn find_string_case_insensitive(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(found) = find_value_case_insensitive(value, key).and_then(|item| item.as_str()) {
            let trimmed = found.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn find_value_case_insensitive<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let object = value.as_object()?;
    if let Some(found) = object
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
    {
        return Some(found);
    }

    object
        .values()
        .find_map(|nested| find_value_case_insensitive(nested, key))
}
