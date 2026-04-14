use super::NodeExecutor;
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct EndExecutor;

impl NodeExecutor for EndExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::End
    }

    fn execute(
        &self,
        _node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        Ok(NodeExecutionResult::success(context.input.clone()).into_terminal())
    }
}
