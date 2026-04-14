use super::NodeExecutor;
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct StartExecutor;

impl NodeExecutor for StartExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Start
    }

    fn execute(
        &self,
        _node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let output = context
            .trigger
            .get("body")
            .cloned()
            .unwrap_or_else(|| context.trigger.clone());
        Ok(NodeExecutionResult::success(output))
    }
}
