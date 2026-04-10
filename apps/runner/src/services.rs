use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::definition::WorkflowDefinition;
use crate::error::RunnerError;
use crate::runtime::NodeExecutionContext;

pub trait FetchConnector: Send + Sync {
    fn name(&self) -> &'static str;
    fn fetch(
        &self,
        request: &Value,
        context: &NodeExecutionContext<'_>,
    ) -> Result<Value, RunnerError>;
}

pub trait ActionHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn execute(
        &self,
        payload: &Value,
        context: &NodeExecutionContext<'_>,
    ) -> Result<Value, RunnerError>;
}

pub trait TaskHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn create(
        &self,
        payload: &Value,
        context: &NodeExecutionContext<'_>,
    ) -> Result<Value, RunnerError>;
}

#[derive(Default, Clone)]
pub struct WorkflowServices {
    pub fetch_connectors: FetchConnectorRegistry,
    pub action_handlers: ActionHandlerRegistry,
    pub task_handlers: TaskHandlerRegistry,
    pub workflow_definitions: WorkflowDefinitionRegistry,
}

impl WorkflowServices {
    pub fn with_defaults() -> Self {
        let mut services = Self::default();
        services.fetch_connectors.register(MockOmsGetOrderConnector);
        services.action_handlers.register(MockRcsDispatchAction);
        services.task_handlers.register(MockManualReviewTaskHandler);
        services
    }
}

#[derive(Default, Clone)]
pub struct FetchConnectorRegistry {
    connectors: HashMap<String, Arc<dyn FetchConnector>>,
}

impl FetchConnectorRegistry {
    pub fn register<C>(&mut self, connector: C)
    where
        C: FetchConnector + 'static,
    {
        self.connectors
            .insert(connector.name().to_string(), Arc::new(connector));
    }

    pub fn resolve(&self, name: &str) -> Option<Arc<dyn FetchConnector>> {
        self.connectors.get(name).cloned()
    }
}

#[derive(Default, Clone)]
pub struct ActionHandlerRegistry {
    handlers: HashMap<String, Arc<dyn ActionHandler>>,
}

impl ActionHandlerRegistry {
    pub fn register<H>(&mut self, handler: H)
    where
        H: ActionHandler + 'static,
    {
        self.handlers
            .insert(handler.name().to_string(), Arc::new(handler));
    }

    pub fn resolve(&self, name: &str) -> Option<Arc<dyn ActionHandler>> {
        self.handlers.get(name).cloned()
    }
}

#[derive(Default, Clone)]
pub struct TaskHandlerRegistry {
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
}

impl TaskHandlerRegistry {
    pub fn register<H>(&mut self, handler: H)
    where
        H: TaskHandler + 'static,
    {
        self.handlers
            .insert(handler.name().to_string(), Arc::new(handler));
    }

    pub fn resolve(&self, name: &str) -> Option<Arc<dyn TaskHandler>> {
        self.handlers.get(name).cloned()
    }
}

#[derive(Default, Clone)]
pub struct WorkflowDefinitionRegistry {
    definitions: HashMap<String, WorkflowDefinition>,
}

impl WorkflowDefinitionRegistry {
    pub fn register(&mut self, key: impl Into<String>, definition: WorkflowDefinition) {
        self.definitions.insert(key.into(), definition);
    }

    pub fn resolve(&self, key: &str) -> Option<WorkflowDefinition> {
        self.definitions.get(key).cloned()
    }
}

struct MockOmsGetOrderConnector;
struct MockRcsDispatchAction;
struct MockManualReviewTaskHandler;

impl FetchConnector for MockOmsGetOrderConnector {
    fn name(&self) -> &'static str {
        "oms.getOrder"
    }

    fn fetch(
        &self,
        request: &Value,
        context: &NodeExecutionContext<'_>,
    ) -> Result<Value, RunnerError> {
        Ok(json!({
            "orderNo": request.get("orderNo").cloned().unwrap_or(Value::Null),
            "warehouseId": request.get("warehouseId").cloned().unwrap_or(Value::Null),
            "fetchedAt": "mocked",
            "status": "loaded",
            "runId": context.run_id,
            "workflowKey": context.workflow_key,
            "workflowVersion": context.workflow_version
        }))
    }
}

impl ActionHandler for MockRcsDispatchAction {
    fn name(&self) -> &'static str {
        "rcs.dispatch"
    }

    fn execute(
        &self,
        payload: &Value,
        context: &NodeExecutionContext<'_>,
    ) -> Result<Value, RunnerError> {
        Ok(json!({
            "accepted": true,
            "dispatchId": format!("dispatch-{}", context.run_id),
            "payload": payload
        }))
    }
}

impl TaskHandler for MockManualReviewTaskHandler {
    fn name(&self) -> &'static str {
        "manual_review"
    }

    fn create(
        &self,
        payload: &Value,
        context: &NodeExecutionContext<'_>,
    ) -> Result<Value, RunnerError> {
        Ok(json!({
            "taskId": format!("task-{}", context.run_id),
            "status": "created",
            "payload": payload
        }))
    }
}
