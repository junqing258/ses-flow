use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::definition::{NodeDefinition, NodeType, WorkflowDefinition};
use crate::error::RunnerError;
use crate::runtime::{NextSignal, NodeExecutionContext, NodeExecutionResult, RunEnvironment, WorkflowRunStatus};
use crate::services::WorkflowServices;
use crate::template::{EvaluationContext, env_to_value, is_truthy, nested_state_patch};

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
        registry.register(FetchExecutor {
            services: services.clone(),
        });
        registry.register(SetStateExecutor);
        registry.register(IfElseExecutor);
        registry.register(SwitchExecutor);
        registry.register(ActionExecutor {
            services: services.clone(),
        });
        registry.register(RespondExecutor);
        registry.register(WaitExecutor);
        registry.register(TaskExecutor {
            services: services.clone(),
        });
        registry.register(SubWorkflowExecutor {
            services,
        });
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

struct StartExecutor;
struct EndExecutor;
struct WebhookTriggerExecutor;
struct FetchExecutor {
    services: Arc<WorkflowServices>,
}
struct SetStateExecutor;
struct IfElseExecutor;
struct SwitchExecutor;
struct ActionExecutor {
    services: Arc<WorkflowServices>,
}
struct RespondExecutor;
struct WaitExecutor;
struct TaskExecutor {
    services: Arc<WorkflowServices>,
}
struct SubWorkflowExecutor {
    services: Arc<WorkflowServices>,
}

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

impl NodeExecutor for WebhookTriggerExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::WebhookTrigger
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let mode = node
            .config
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("body");
        let payload = match mode {
            "full" => context.trigger.clone(),
            "headers" => context
                .trigger
                .get("headers")
                .cloned()
                .unwrap_or(Value::Null),
            _ => context
                .trigger
                .get("body")
                .cloned()
                .unwrap_or_else(|| context.trigger.clone()),
        };

        Ok(NodeExecutionResult::success(payload))
    }
}

impl NodeExecutor for FetchExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Fetch
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let request = resolve_mapping(node, context);
        let connector = node
            .config
            .get("connector")
            .and_then(Value::as_str)
            .unwrap_or("connector.unknown");
        let connector_impl = self
            .services
            .fetch_connectors
            .resolve(connector)
            .ok_or_else(|| RunnerError::MissingFetchConnector(connector.to_string()))?;
        let data = connector_impl.fetch(&request, context)?;

        Ok(NodeExecutionResult::success(json!({
            "connector": connector,
            "request": request,
            "data": data
        })))
    }
}

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
            .and_then(Value::as_str)
            .unwrap_or("statePatch");
        let state_value = payload.get("value").cloned().unwrap_or(payload.clone());

        Ok(NodeExecutionResult::success(state_value.clone())
            .with_state_patch(nested_state_patch(state_path, state_value)))
    }
}

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
        let resolved = template_context.resolve_value(&expression);
        let expected = node
            .config
            .get("equals")
            .map(|value| template_context.resolve_value(value));
        let condition = match expected {
            Some(expected) => resolved == expected,
            None => is_truthy(&resolved),
        };
        let branch_key = if condition { "then" } else { "else" };

        Ok(NodeExecutionResult::success(json!({
            "condition": condition,
            "value": resolved
        }))
        .with_branch_key(branch_key))
    }
}

impl NodeExecutor for SwitchExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Switch
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let template_context = EvaluationContext {
            trigger: context.trigger,
            input: context.input,
            state: context.state,
            env: env_to_value(context.env),
            output: &Value::Null,
        };

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

        Ok(
            NodeExecutionResult::success(json!({ "branch": branch_key }))
                .with_branch_key(branch_key),
        )
    }
}

impl NodeExecutor for ActionExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Action
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let action = node
            .config
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("action.unknown");
        let handler = self
            .services
            .action_handlers
            .resolve(action)
            .ok_or_else(|| RunnerError::MissingActionHandler(action.to_string()))?;
        let response = handler.execute(&payload, context)?;

        Ok(NodeExecutionResult::success(json!({
            "action": action,
            "response": response
        })))
    }
}

impl NodeExecutor for RespondExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Respond
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let status_code = node
            .config
            .get("statusCode")
            .and_then(Value::as_u64)
            .unwrap_or(200);
        let response = json!({
            "statusCode": status_code,
            "body": payload
        });

        let mut result = NodeExecutionResult::success(payload).with_signal(NextSignal {
            signal_type: "webhook_response".to_string(),
            payload: response,
        });

        if node
            .config
            .get("terminal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            result = result.into_terminal();
        }

        Ok(result)
    }
}

impl NodeExecutor for WaitExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Wait
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let event = node
            .config
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("external_callback");

        Ok(NodeExecutionResult::waiting(
            NextSignal {
                signal_type: event.to_string(),
                payload: payload.clone(),
            },
            json!({
                "event": event,
                "payload": payload
            }),
        ))
    }
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
            .and_then(Value::as_str)
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

        let engine = crate::engine::WorkflowEngine::with_services((*self.services).clone());
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
            "timeline": summary.timeline
        });

        let export_path = node.config.get("statePath").and_then(Value::as_str);

        match summary.status {
            WorkflowRunStatus::Completed => {
                let mut result = NodeExecutionResult::success(output.clone());
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
            WorkflowRunStatus::Failed => Ok(NodeExecutionResult::failed(
                "sub_workflow_failed",
                format!("sub-workflow {} failed", definition.meta.key),
                false,
            )),
            WorkflowRunStatus::Waiting => Ok(NodeExecutionResult::failed(
                "sub_workflow_waiting_not_supported",
                format!(
                    "sub-workflow {} entered waiting state which is not yet resumable from parent",
                    definition.meta.key
                ),
                false,
            )),
        }
    }
}

fn resolve_mapping(node: &NodeDefinition, context: &NodeExecutionContext<'_>) -> Value {
    let template_context = evaluation_context(context, &Value::Null);

    if node.input_mapping.is_null() {
        return context.input.clone();
    }

    template_context.resolve_value(&node.input_mapping)
}

fn evaluation_context<'a>(
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

fn resolve_sub_workflow_definition(
    node: &NodeDefinition,
    services: &WorkflowServices,
) -> Result<WorkflowDefinition, RunnerError> {
    if let Some(definition) = node
        .config
        .get("definition")
        .cloned()
        .or_else(|| node.config.get("workflow").cloned())
    {
        return serde_json::from_value(definition)
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

fn clone_env(env: &RunEnvironment) -> RunEnvironment {
    RunEnvironment {
        tenant_id: env.tenant_id.clone(),
        warehouse_id: env.warehouse_id.clone(),
        operator_id: env.operator_id.clone(),
    }
}
