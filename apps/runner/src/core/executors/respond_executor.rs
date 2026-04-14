use serde_json::json;

use super::{NodeExecutor, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NextSignal, NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct RespondExecutor;

impl NodeExecutor for RespondExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Respond
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let status_code = node
            .config
            .get("statusCode")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(200);
        let response = json!({
            "statusCode": status_code,
            "body": payload
        });

        let mut result = NodeExecutionResult::success(payload).with_signal(NextSignal {
            signal_type: "webhook_response".to_string(),
            payload: response,
        });

        if node
            .config
            .get("terminal")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            result = result.into_terminal();
        }

        Ok(result)
    }
}
