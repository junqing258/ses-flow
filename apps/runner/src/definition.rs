use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::RunnerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub meta: WorkflowMeta,
    pub trigger: TriggerDefinition,
    #[serde(rename = "inputSchema", default)]
    pub input_schema: Value,
    pub nodes: Vec<NodeDefinition>,
    pub transitions: Vec<TransitionDefinition>,
    #[serde(default)]
    pub policies: WorkflowPolicies,
}

impl WorkflowDefinition {
    pub fn validate(&self) -> Result<(), RunnerError> {
        if self.nodes.is_empty() {
            return Err(RunnerError::Validation(
                "workflow must contain at least one node".to_string(),
            ));
        }

        let mut unique_ids = HashSet::new();
        let mut start_nodes = 0usize;

        for node in &self.nodes {
            if !unique_ids.insert(node.id.as_str()) {
                return Err(RunnerError::Validation(format!(
                    "duplicated node id detected: {}",
                    node.id
                )));
            }

            if node.node_type == NodeType::Start {
                start_nodes += 1;
            }
        }

        if start_nodes != 1 {
            return Err(RunnerError::Validation(format!(
                "workflow must contain exactly one start node, found {start_nodes}"
            )));
        }

        for transition in &self.transitions {
            if self.node(&transition.from).is_none() {
                return Err(RunnerError::Validation(format!(
                    "transition source node does not exist: {}",
                    transition.from
                )));
            }

            if self.node(&transition.to).is_none() {
                return Err(RunnerError::Validation(format!(
                    "transition target node does not exist: {}",
                    transition.to
                )));
            }
        }

        Ok(())
    }

    pub fn node(&self, node_id: &str) -> Option<&NodeDefinition> {
        self.nodes.iter().find(|node| node.id == node_id)
    }

    pub fn start_node(&self) -> Result<&NodeDefinition, RunnerError> {
        self.nodes
            .iter()
            .find(|node| node.node_type == NodeType::Start)
            .ok_or_else(|| RunnerError::Validation("start node is required".to_string()))
    }

    pub fn transitions_from(&self, node_id: &str) -> Vec<&TransitionDefinition> {
        let mut transitions = self
            .transitions
            .iter()
            .filter(|transition| transition.from == node_id)
            .collect::<Vec<_>>();

        transitions.sort_by_key(|transition| std::cmp::Reverse(transition.priority.unwrap_or(0)));
        transitions
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMeta {
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    pub version: u32,
    #[serde(default)]
    pub scope: WorkflowScope,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowScope {
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub customer: Option<String>,
    #[serde(default)]
    pub warehouse: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDefinition {
    #[serde(rename = "type")]
    pub trigger_type: TriggerType,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(rename = "eventName", default)]
    pub event_name: Option<String>,
    #[serde(rename = "responseMode", default)]
    pub response_mode: Option<ResponseMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Webhook,
    Manual,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    Sync,
    AsyncAck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub name: String,
    #[serde(default)]
    pub config: Value,
    #[serde(rename = "inputMapping", default)]
    pub input_mapping: Value,
    #[serde(rename = "outputMapping", default)]
    pub output_mapping: Value,
    #[serde(rename = "timeoutMs", default)]
    pub timeout_ms: Option<u64>,
    #[serde(rename = "retryPolicy", default)]
    pub retry_policy: Option<RetryPolicy>,
    #[serde(rename = "onError", default)]
    pub on_error: Option<OnErrorPolicy>,
    #[serde(default)]
    pub annotations: HashMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Start,
    End,
    Fetch,
    SetState,
    Switch,
    Action,
    Wait,
    Task,
    Respond,
    Code,
    SubWorkflow,
    WebhookTrigger,
    IfElse,
}

impl NodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Fetch => "fetch",
            Self::SetState => "set_state",
            Self::Switch => "switch",
            Self::Action => "action",
            Self::Wait => "wait",
            Self::Task => "task",
            Self::Respond => "respond",
            Self::Code => "code",
            Self::SubWorkflow => "sub_workflow",
            Self::WebhookTrigger => "webhook_trigger",
            Self::IfElse => "if_else",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDefinition {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(rename = "branchType", default)]
    pub branch_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowPolicies {
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
    #[serde(default)]
    pub idempotency: Option<IdempotencyPolicy>,
    #[serde(default)]
    pub audit_level: Option<String>,
    #[serde(default)]
    pub data_retention: Option<String>,
    #[serde(rename = "allowManualRetry", default)]
    pub allow_manual_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default)]
    pub max_attempts: Option<u32>,
    #[serde(default)]
    pub backoff_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyPolicy {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnErrorPolicy {
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(rename = "nextNodeId", default)]
    pub next_node_id: Option<String>,
}
