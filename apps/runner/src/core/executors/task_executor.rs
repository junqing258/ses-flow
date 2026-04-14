use std::sync::Arc;

use serde_json::json;

use super::{NodeExecutor, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NextSignal, NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;
use crate::services::WorkflowServices;

pub(super) struct TaskExecutor {
    pub(super) services: Arc<WorkflowServices>,
}

impl NodeExecutor for TaskExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Task
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let task_type = node
            .config
            .get("taskType")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("generic_task");
        let handler = self
            .services
            .task_handlers
            .resolve(task_type)
            .ok_or_else(|| RunnerError::MissingTaskHandler(task_type.to_string()))?;
        let task = handler.create(&payload, context)?;

        Ok(NodeExecutionResult::waiting(
            NextSignal {
                signal_type: "task_created".to_string(),
                payload: task.clone(),
            },
            json!({
                "taskType": task_type,
                "task": task
            }),
        ))
    }
}
