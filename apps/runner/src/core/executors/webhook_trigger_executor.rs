use serde_json::Value;

use super::NodeExecutor;
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct WebhookTriggerExecutor;

impl NodeExecutor for WebhookTriggerExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::WebhookTrigger
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let mode = node.config.get("mode").and_then(Value::as_str).unwrap_or("body");
        let payload = match mode {
            "full" => context.trigger.clone(),
            "headers" => context.trigger.get("headers").cloned().unwrap_or(Value::Null),
            _ => context
                .trigger
                .get("body")
                .cloned()
                .unwrap_or_else(|| context.trigger.clone()),
        };

        Ok(NodeExecutionResult::success(payload))
    }
}
