pub mod definition;
pub mod engine;
pub mod executors;
pub mod runtime;
pub mod template;

pub use definition::{WorkflowDefinition, NodeDefinition, NodeType, TransitionDefinition};
pub use engine::{WorkflowEngine, new_run_id};
pub use executors::{NodeExecutor, ExecutorRegistry};
pub use runtime::{
    RunEnvironment, NodeExecutionContext, NodeExecutionResult, NodeExecutionRecord,
    NodeLogRecord, NextSignal, WorkflowRunSummary, WorkflowRunStatus, WorkflowRunEvent,
    WorkflowRunObserver, NoopWorkflowRunObserver, ExecutionStatus, WorkflowRunSnapshot,
};
pub use template::{EvaluationContext, is_truthy, env_to_value, nested_state_patch, merge_state};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod template_tests;
