use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

pub fn deserialize_workflow_definition(value: Value) -> Result<WorkflowDefinition, RunnerError> {
    serde_json::from_value(normalize_workflow_definition_value(value)).map_err(RunnerError::Json)
}

fn normalize_workflow_definition_value(mut value: Value) -> Value {
    let Some(nodes) = value.get_mut("nodes").and_then(Value::as_array_mut) else {
        return value;
    };

    for node in nodes {
        normalize_node_definition_value(node);
    }

    value
}

fn normalize_node_definition_value(node: &mut Value) {
    let Some(node_object) = node.as_object_mut() else {
        return;
    };

    if let Some(node_type) = node_object.get_mut("type") {
        if matches!(node_type.as_str(), Some("action" | "command")) {
            *node_type = Value::String("shell".to_string());
        }
    }

    if let Some(config) = node_object.get_mut("config").and_then(Value::as_object_mut) {
        if let Some(definition) = config.get_mut("definition") {
            *definition = normalize_workflow_definition_value(definition.clone());
        }

        if let Some(workflow) = config.get_mut("workflow") {
            *workflow = normalize_workflow_definition_value(workflow.clone());
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeType {
    Start,
    End,
    Fetch,
    DbQuery,
    SetState,
    Switch,
    Shell,
    Wait,
    Respond,
    Code,
    SubWorkflow,
    WebhookTrigger,
    IfElse,
    Plugin(String),
    Custom(String),
}

impl NodeType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Fetch => "fetch",
            Self::DbQuery => "db_query",
            Self::SetState => "set_state",
            Self::Switch => "switch",
            Self::Shell => "shell",
            Self::Wait => "wait",
            Self::Respond => "respond",
            Self::Code => "code",
            Self::SubWorkflow => "sub_workflow",
            Self::WebhookTrigger => "webhook_trigger",
            Self::IfElse => "if_else",
            Self::Plugin(value) | Self::Custom(value) => value.as_str(),
        }
    }

    pub fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin(_))
    }
}

impl Serialize for NodeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for NodeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(match raw.as_str() {
            "start" => Self::Start,
            "end" => Self::End,
            "fetch" => Self::Fetch,
            "db_query" | "db" | "database" => Self::DbQuery,
            "set_state" => Self::SetState,
            "switch" => Self::Switch,
            "shell" | "action" | "command" => Self::Shell,
            "wait" => Self::Wait,
            "respond" => Self::Respond,
            "code" => Self::Code,
            "sub_workflow" | "subworkflow" => Self::SubWorkflow,
            "webhook_trigger" | "webhook" => Self::WebhookTrigger,
            "if_else" | "if" => Self::IfElse,
            _ if raw.starts_with("plugin:") => Self::Plugin(raw),
            _ => Self::Custom(raw),
        })
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
