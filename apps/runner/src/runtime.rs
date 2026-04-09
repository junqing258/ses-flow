use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::definition::NodeType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEnvironment {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "warehouseId", default)]
    pub warehouse_id: Option<String>,
    #[serde(rename = "operatorId", default)]
    pub operator_id: Option<String>,
}

impl Default for RunEnvironment {
    fn default() -> Self {
        Self {
            tenant_id: "tenant-a".to_string(),
            warehouse_id: Some("WH-1".to_string()),
            operator_id: Some("system".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeExecutionContext<'a> {
    pub run_id: &'a str,
    pub workflow_key: &'a str,
    pub workflow_version: u32,
    pub trigger: &'a Value,
    pub input: &'a Value,
    pub state: &'a Value,
    pub env: &'a RunEnvironment,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeExecutionResult {
    pub status: ExecutionStatus,
    pub output: Value,
    #[serde(rename = "statePatch")]
    pub state_patch: Value,
    #[serde(rename = "branchKey", skip_serializing_if = "Option::is_none")]
    pub branch_key: Option<String>,
    #[serde(rename = "nextSignal", skip_serializing_if = "Option::is_none")]
    pub next_signal: Option<NextSignal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<NodeExecutionError>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub terminal: bool,
}

impl NodeExecutionResult {
    pub fn success(output: Value) -> Self {
        Self {
            status: ExecutionStatus::Success,
            output,
            state_patch: Value::Null,
            branch_key: None,
            next_signal: None,
            error: None,
            terminal: false,
        }
    }

    pub fn waiting(signal: NextSignal, output: Value) -> Self {
        Self {
            status: ExecutionStatus::Waiting,
            output,
            state_patch: Value::Null,
            branch_key: None,
            next_signal: Some(signal),
            error: None,
            terminal: false,
        }
    }

    pub fn failed(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            status: ExecutionStatus::Failed,
            output: Value::Null,
            state_patch: Value::Null,
            branch_key: None,
            next_signal: None,
            error: Some(NodeExecutionError {
                code: code.into(),
                message: message.into(),
                retryable,
                details: Value::Null,
            }),
            terminal: false,
        }
    }

    pub fn with_state_patch(mut self, state_patch: Value) -> Self {
        self.state_patch = state_patch;
        self
    }

    pub fn with_branch_key(mut self, branch_key: impl Into<String>) -> Self {
        self.branch_key = Some(branch_key.into());
        self
    }

    pub fn into_terminal(mut self) -> Self {
        self.terminal = true;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Success,
    Waiting,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextSignal {
    #[serde(rename = "type")]
    pub signal_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunSummary {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
    pub status: WorkflowRunStatus,
    #[serde(rename = "currentNodeId", skip_serializing_if = "Option::is_none")]
    pub current_node_id: Option<String>,
    pub state: Value,
    pub timeline: Vec<NodeExecutionRecord>,
    #[serde(rename = "lastSignal", skip_serializing_if = "Option::is_none")]
    pub last_signal: Option<NextSignal>,
    #[serde(rename = "resumeState", skip_serializing_if = "Option::is_none")]
    pub resume_state: Option<WorkflowRunSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Completed,
    Waiting,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionRecord {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "nodeType")]
    pub node_type: NodeType,
    pub status: ExecutionStatus,
    pub output: Value,
    #[serde(rename = "statePatch")]
    pub state_patch: Value,
    #[serde(rename = "branchKey", skip_serializing_if = "Option::is_none")]
    pub branch_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunSnapshot {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
    #[serde(rename = "currentNodeId")]
    pub current_node_id: String,
    pub trigger: Value,
    #[serde(rename = "lastInput")]
    pub last_input: Value,
    pub state: Value,
    pub timeline: Vec<NodeExecutionRecord>,
    #[serde(rename = "lastSignal", skip_serializing_if = "Option::is_none")]
    pub last_signal: Option<NextSignal>,
    pub env: RunEnvironment,
}
