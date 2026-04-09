use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::definition::{NodeDefinition, NodeType};
use crate::error::RunnerError;
use crate::runtime::{NextSignal, NodeExecutionContext, NodeExecutionResult};
use crate::services::WorkflowServices;
use crate::template::{EvaluationContext, env_to_value, nested_state_patch};

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
        registry.register(FetchExecutor {
            services: services.clone(),
        });
        registry.register(SetStateExecutor);
        registry.register(SwitchExecutor);
        registry.register(ActionExecutor {
            services: services.clone(),
        });
        registry.register(WaitExecutor);
        registry.register(TaskExecutor { services });
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
struct FetchExecutor {
    services: Arc<WorkflowServices>,
}
struct SetStateExecutor;
struct SwitchExecutor;
struct ActionExecutor {
    services: Arc<WorkflowServices>,
}
struct WaitExecutor;
struct TaskExecutor {
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
            env: &env_to_value(context.env),
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

fn resolve_mapping(node: &NodeDefinition, context: &NodeExecutionContext<'_>) -> Value {
    let env = env_to_value(context.env);
    let template_context = EvaluationContext {
        trigger: context.trigger,
        input: context.input,
        state: context.state,
        env: &env,
        output: &Value::Null,
    };

    if node.input_mapping.is_null() {
        return context.input.clone();
    }

    template_context.resolve_value(&node.input_mapping)
}
