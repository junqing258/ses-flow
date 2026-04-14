use serde_json::json;

use super::{NodeExecutor, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NextSignal, NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct WaitExecutor;

impl NodeExecutor for WaitExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Wait
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let event = node
            .config
            .get("event")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("external_callback");

        Ok(NodeExecutionResult::waiting(
            NextSignal {
                signal_type: event.to_string(),
                payload: payload.clone(),
            },
            json!({
                "event": event,
                "payload": payload
            }),
        ))
    }
}
