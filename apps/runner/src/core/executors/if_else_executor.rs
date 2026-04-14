use serde_json::{Value, json};

use super::{NodeExecutor, evaluation_context};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::core::template::is_truthy;
use crate::error::RunnerError;

pub(super) struct IfElseExecutor;

impl NodeExecutor for IfElseExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::IfElse
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
            .unwrap_or_else(|| Value::Bool(false));
        let evaluated = template_context.resolve_value(&expression);
        let branch_key = if is_truthy(&evaluated) { "then" } else { "else" };

        Ok(NodeExecutionResult::success(json!({
            "branch": branch_key,
            "matched": branch_key == "then"
        }))
        .with_branch_key(branch_key))
    }
}
