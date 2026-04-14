use super::{NodeExecutor, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::core::template::nested_state_patch;
use crate::error::RunnerError;

pub(super) struct SetStateExecutor;

impl NodeExecutor for SetStateExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::SetState
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let state_path = node
            .config
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("statePatch");
        let state_value = payload.get("value").cloned().unwrap_or(payload.clone());

        Ok(NodeExecutionResult::success(state_value.clone())
            .with_state_patch(nested_state_patch(state_path, state_value)))
    }
}
