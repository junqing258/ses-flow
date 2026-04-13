pub mod definition;
pub mod engine;
pub mod executors;
pub mod runtime;
pub mod template;

pub use definition::{
    NodeDefinition, NodeType, TransitionDefinition, WorkflowDefinition,
    deserialize_workflow_definition,
};
pub use engine::{WorkflowEngine, new_run_id};
pub use executors::{ExecutorRegistry, NodeExecutor};
pub use runtime::{
    ExecutionStatus, NextSignal, NodeExecutionContext, NodeExecutionRecord, NodeExecutionResult,
    NodeLogRecord, NoopWorkflowRunController, NoopWorkflowRunObserver, RunEnvironment,
    WorkflowRunController, WorkflowRunEvent, WorkflowRunObserver, WorkflowRunSnapshot,
    WorkflowRunStatus, WorkflowRunSummary,
};
pub use template::{EvaluationContext, env_to_value, is_truthy, merge_state, nested_state_patch};

#[cfg(test)]
mod template_tests;
#[cfg(test)]
mod tests;
