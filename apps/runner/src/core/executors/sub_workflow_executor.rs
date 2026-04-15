use std::sync::Arc;

use serde_json::{Value, json};

use super::{NodeExecutor, clone_env, resolve_mapping, resolve_sub_workflow_definition};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NextSignal, NodeExecutionContext, NodeExecutionResult, WorkflowRunStatus};
use crate::core::template::nested_state_patch;
use crate::error::RunnerError;
use crate::services::WorkflowServices;

pub(super) struct SubWorkflowExecutor {
    pub(super) services: Arc<WorkflowServices>,
}

impl NodeExecutor for SubWorkflowExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::SubWorkflow
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let input = resolve_mapping(node, context);
        let definition = resolve_sub_workflow_definition(node, &self.services)?;
        definition.validate()?;

        let engine = super::super::WorkflowEngine::with_services((*self.services).clone());
        let nested_trigger = json!({
            "headers": {
                "parentRunId": context.run_id,
                "parentWorkflowKey": context.workflow_key,
                "parentNodeId": node.id
            },
            "body": input
        });
        let summary = engine.run(&definition, nested_trigger, clone_env(context.env))?;

        let output = json!({
            "workflowKey": summary.workflow_key,
            "workflowVersion": summary.workflow_version,
            "runId": summary.run_id,
            "status": summary.status,
            "state": summary.state,
            "timeline": summary.timeline,
            "lastSignal": summary.last_signal,
            "resumeState": summary.resume_state
        });

        let export_path = node.config.get("statePath").and_then(Value::as_str);

        match summary.status {
            WorkflowRunStatus::Running => Err(RunnerError::SubWorkflow(format!(
                "sub-workflow {} returned unexpected running status",
                definition.meta.key
            ))),
            WorkflowRunStatus::Completed => {
                let mut result = NodeExecutionResult::success(output.clone());
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
            WorkflowRunStatus::Failed => {
                let mut result = NodeExecutionResult::failed(
                    "sub_workflow_failed",
                    format!("sub-workflow {} failed", definition.meta.key),
                    false,
                );
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
            WorkflowRunStatus::Waiting => {
                let nested_signal = summary.last_signal.clone().unwrap_or(NextSignal {
                    signal_type: "sub_workflow_waiting".to_string(),
                    payload: json!({
                        "childWorkflowKey": summary.workflow_key,
                        "childRunId": summary.run_id
                    }),
                });
                let waiting_output = output.clone();
                let mut result = NodeExecutionResult::waiting(nested_signal, output);
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, waiting_output));
                }
                Ok(result)
            }
            WorkflowRunStatus::Terminated => {
                let mut result = NodeExecutionResult::failed(
                    "sub_workflow_terminated",
                    format!("sub-workflow {} was terminated", definition.meta.key),
                    false,
                );
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
        }
    }
}
