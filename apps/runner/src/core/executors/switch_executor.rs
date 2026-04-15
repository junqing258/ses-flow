use serde_json::{Value, json};

use super::{NodeExecutor, evaluation_context};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct SwitchExecutor;

impl NodeExecutor for SwitchExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Switch
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let template_context = evaluation_context(context, &Value::Null);
        let expression = node
            .config
            .get("expression")
            .cloned()
            .unwrap_or_else(|| Value::String("default".to_string()));
        let branch_value = template_context.resolve_value(&expression);
        let branch_key = match branch_value {
            Value::String(value) => value,
            Value::Null => "default".to_string(),
            other => other.to_string(),
        };

        Ok(NodeExecutionResult::success(json!({ "branch": branch_key })).with_branch_key(branch_key))
    }
}
