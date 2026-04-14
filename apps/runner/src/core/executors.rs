mod code_executor;
mod end_executor;
mod fetch_executor;
mod if_else_executor;
mod respond_executor;
mod set_state_executor;
mod shell_executor;
mod start_executor;
mod sub_workflow_executor;
mod switch_executor;
mod task_executor;
mod wait_executor;
mod webhook_trigger_executor;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::definition::{
    NodeDefinition, NodeType, WorkflowDefinition, deserialize_workflow_definition,
};
use super::runtime::{NodeExecutionContext, NodeExecutionResult, RunEnvironment};
use super::template::{EvaluationContext, env_to_value};
use crate::error::RunnerError;
use crate::services::WorkflowServices;

use self::code_executor::CodeExecutor;
use self::end_executor::EndExecutor;
use self::fetch_executor::FetchExecutor;
use self::if_else_executor::IfElseExecutor;
use self::respond_executor::RespondExecutor;
use self::set_state_executor::SetStateExecutor;
use self::shell_executor::ShellExecutor;
use self::start_executor::StartExecutor;
use self::sub_workflow_executor::SubWorkflowExecutor;
use self::switch_executor::SwitchExecutor;
use self::task_executor::TaskExecutor;
use self::wait_executor::WaitExecutor;
use self::webhook_trigger_executor::WebhookTriggerExecutor;

// region 执行器抽象与注册表
pub trait NodeExecutor: Send + Sync {
    fn node_type(&self) -> NodeType;
    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError>;
}

#[derive(Default)]
pub struct ExecutorRegistry {
    executors: HashMap<NodeType, Arc<dyn NodeExecutor>>,
}

impl ExecutorRegistry {
    pub fn with_defaults(services: Arc<WorkflowServices>) -> Self {
        let mut registry = Self::default();
        registry.register(StartExecutor);
        registry.register(EndExecutor);
        registry.register(WebhookTriggerExecutor);
        registry.register(FetchExecutor);
        registry.register(SetStateExecutor);
        registry.register(IfElseExecutor);
        registry.register(SwitchExecutor);
        registry.register(CodeExecutor);
        registry.register(ShellExecutor);
        registry.register(RespondExecutor);
        registry.register(WaitExecutor);
        registry.register(TaskExecutor {
            services: services.clone(),
        });
        registry.register(SubWorkflowExecutor { services });
        registry
    }

    pub fn register<E>(&mut self, executor: E)
    where
        E: NodeExecutor + 'static,
    {
        self.executors
            .insert(executor.node_type(), Arc::new(executor));
    }

    pub fn resolve(&self, node_type: NodeType) -> Option<Arc<dyn NodeExecutor>> {
        self.executors.get(&node_type).cloned()
    }
}
// endregion 执行器抽象与注册表

// region 节点执行器实现位于独立模块
// endregion 节点执行器实现位于独立模块

// region 通用解析辅助函数
pub(super) fn resolve_mapping(node: &NodeDefinition, context: &NodeExecutionContext<'_>) -> Value {
    let template_context = evaluation_context(context, &Value::Null);

    if node.input_mapping.is_null() {
        return context.input.clone();
    }

    template_context.resolve_value(&node.input_mapping)
}

pub(super) fn resolve_config(
    node: &NodeDefinition,
    context: &NodeExecutionContext<'_>,
    output: &Value,
) -> Value {
    evaluation_context(context, output).resolve_value(&node.config)
}
// endregion 通用解析辅助函数

pub(super) fn wait_for_process_output(
    mut child: std::process::Child,
    timeout_ms: Option<u64>,
    process_name: &str,
) -> Result<std::process::Output, RunnerError> {
    let Some(timeout_ms) = timeout_ms else {
        return child
            .wait_with_output()
            .map_err(|error| match process_name {
                "shell" => RunnerError::ShellExecution(error.to_string()),
                _ => RunnerError::CodeExecution(error.to_string()),
            });
    };

    let started_at = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait().map_err(|error| match process_name {
            "shell" => RunnerError::ShellExecution(error.to_string()),
            _ => RunnerError::CodeExecution(error.to_string()),
        })? {
            Some(_) => {
                return child
                    .wait_with_output()
                    .map_err(|error| match process_name {
                        "shell" => RunnerError::ShellExecution(error.to_string()),
                        _ => RunnerError::CodeExecution(error.to_string()),
                    });
            }
            None if started_at.elapsed() >= timeout => {
                child.kill().map_err(|error| match process_name {
                    "shell" => RunnerError::ShellExecution(error.to_string()),
                    _ => RunnerError::CodeExecution(error.to_string()),
                })?;
                let _ = child.wait();
                let message = format!("{process_name} node exceeded timeout of {timeout_ms}ms");
                return Err(match process_name {
                    "shell" => RunnerError::ShellExecution(message),
                    _ => RunnerError::CodeExecution(message),
                });
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

// region 上下文与子流程辅助函数
pub(super) fn evaluation_context<'a>(
    context: &'a NodeExecutionContext<'a>,
    output: &'a Value,
) -> EvaluationContext<'a> {
    let env = env_to_value(context.env);
    EvaluationContext {
        trigger: context.trigger,
        input: context.input,
        state: context.state,
        env,
        output,
    }
}

pub(super) fn resolve_sub_workflow_definition(
    node: &NodeDefinition,
    services: &WorkflowServices,
) -> Result<WorkflowDefinition, RunnerError> {
    if let Some(definition) = node
        .config
        .get("definition")
        .cloned()
        .or_else(|| node.config.get("workflow").cloned())
    {
        return deserialize_workflow_definition(definition)
            .map_err(|error| RunnerError::SubWorkflow(error.to_string()));
    }

    if let Some(reference) = node
        .config
        .get("ref")
        .and_then(Value::as_str)
        .or_else(|| node.config.get("workflowKey").and_then(Value::as_str))
    {
        return services
            .workflow_definitions
            .resolve(reference)
            .ok_or_else(|| RunnerError::MissingSubWorkflow(reference.to_string()));
    }

    Err(RunnerError::SubWorkflow(format!(
        "node {} is missing sub-workflow definition/ref",
        node.id
    )))
}

pub(super) fn clone_env(env: &RunEnvironment) -> RunEnvironment {
    RunEnvironment {
        tenant_id: env.tenant_id.clone(),
        warehouse_id: env.warehouse_id.clone(),
        operator_id: env.operator_id.clone(),
    }
}
// endregion 上下文与子流程辅助函数
