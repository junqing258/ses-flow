use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;
use thiserror::Error;
use tokio::sync::broadcast;

use crate::core::definition::WorkflowDefinition;
use crate::core::engine::{WorkflowEngine, new_run_id};
use crate::error::RunnerError;
use crate::core::runtime::{
    RunEnvironment, WorkflowRunEvent, WorkflowRunObserver, WorkflowRunStatus, WorkflowRunSummary,
};
use crate::store::{InMemoryRunStore, WorkflowRunStore, WorkflowRunner};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Runner(#[from] RunnerError),
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRecord {
    pub id: String,
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRegistration {
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
}

#[derive(Clone)]
pub struct WorkflowServer {
    store: Arc<dyn WorkflowRunStore>,
    runner: Arc<WorkflowRunner>,
    catalog: WorkflowCatalog,
    run_registry: RunRegistry,
    events: broadcast::Sender<WorkflowRunEvent>,
}

impl WorkflowServer {
    pub fn new() -> Self {
        let store: Arc<dyn WorkflowRunStore> = Arc::new(InMemoryRunStore::new());
        let (events, _) = broadcast::channel(256);
        let observer = Arc::new(BroadcastRunObserver {
            store: store.clone(),
            events: events.clone(),
        });
        let runner = Arc::new(WorkflowRunner::new(
            WorkflowEngine::with_observer(observer),
            store.clone(),
        ));

        Self {
            store,
            runner,
            catalog: WorkflowCatalog::new(),
            run_registry: RunRegistry::default(),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowRunEvent> {
        self.events.subscribe()
    }

    pub fn register_workflow(
        &self,
        workspace_id: Option<String>,
        workspace_name: Option<String>,
        definition: WorkflowDefinition,
    ) -> Result<WorkflowRegistration, ServerError> {
        definition.validate()?;
        let workspace = self.catalog.ensure_workspace(workspace_id, workspace_name);
        let workflow = self.catalog.register_workflow(&workspace.id, definition);

        Ok(WorkflowRegistration {
            workspace_id: workflow.workspace_id,
            workflow_id: workflow.id,
            workflow_key: workflow.workflow_key,
            workflow_version: workflow.workflow_version,
        })
    }

    pub fn get_workflow(&self, workflow_id: &str) -> Result<WorkflowRecord, ServerError> {
        self.catalog
            .get_workflow(workflow_id)
            .ok_or_else(|| ServerError::NotFound(format!("workflow not found: {workflow_id}")))
    }

    pub fn get_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, ServerError> {
        Ok(self.store.load_summary(run_id)?)
    }

    pub async fn start_workflow(
        &self,
        workflow_id: &str,
        trigger: serde_json::Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, ServerError> {
        let workflow = self
            .catalog
            .get_definition(workflow_id)
            .ok_or_else(|| ServerError::NotFound(format!("workflow not found: {workflow_id}")))?;
        let run_id = new_run_id();
        let start_node = workflow.definition.start_node()?.id.clone();

        self.run_registry.bind(&run_id, workflow.id.clone());

        let summary = WorkflowRunSummary {
            run_id: run_id.clone(),
            workflow_key: workflow.definition.meta.key.clone(),
            workflow_version: workflow.definition.meta.version,
            status: WorkflowRunStatus::Running,
            current_node_id: Some(start_node),
            state: json!({}),
            timeline: Vec::new(),
            last_signal: None,
            resume_state: None,
        };
        self.publish_summary(&summary);

        let runner = self.runner.clone();
        let fallback = self.clone();
        tokio::task::spawn_blocking(move || {
            let run_result = runner.run_with_id(&workflow.definition, run_id.clone(), trigger, env);
            if let Err(error) = run_result {
                fallback.publish_summary(&WorkflowRunSummary {
                    run_id: run_id.clone(),
                    workflow_key: workflow.definition.meta.key.clone(),
                    workflow_version: workflow.definition.meta.version,
                    status: WorkflowRunStatus::Failed,
                    current_node_id: None,
                    state: json!({
                        "error": error.to_string()
                    }),
                    timeline: Vec::new(),
                    last_signal: None,
                    resume_state: None,
                });
            }
        });

        Ok(summary)
    }

    pub async fn resume_workflow(
        &self,
        run_id: &str,
        event: serde_json::Value,
    ) -> Result<WorkflowRunSummary, ServerError> {
        let workflow_id = self
            .run_registry
            .resolve(run_id)
            .ok_or_else(|| ServerError::NotFound(format!("workflow run not found: {run_id}")))?;
        let workflow = self
            .catalog
            .get_definition(&workflow_id)
            .ok_or_else(|| ServerError::NotFound(format!("workflow not found: {workflow_id}")))?;

        let running_summary = WorkflowRunSummary {
            run_id: run_id.to_string(),
            workflow_key: workflow.definition.meta.key.clone(),
            workflow_version: workflow.definition.meta.version,
            status: WorkflowRunStatus::Running,
            current_node_id: self
                .store
                .load_summary(run_id)?
                .and_then(|summary| summary.current_node_id),
            state: self
                .store
                .load_summary(run_id)?
                .map(|summary| summary.state)
                .unwrap_or_else(|| json!({})),
            timeline: self
                .store
                .load_summary(run_id)?
                .map(|summary| summary.timeline)
                .unwrap_or_default(),
            last_signal: None,
            resume_state: None,
        };
        self.publish_summary(&running_summary);

        let runner = self.runner.clone();
        let fallback = self.clone();
        let run_id = run_id.to_string();
        tokio::task::spawn_blocking(move || {
            let resume_result = runner.resume_by_run_id(&workflow.definition, &run_id, event);
            if let Err(error) = resume_result {
                fallback.publish_summary(&WorkflowRunSummary {
                    run_id: run_id.clone(),
                    workflow_key: workflow.definition.meta.key.clone(),
                    workflow_version: workflow.definition.meta.version,
                    status: WorkflowRunStatus::Failed,
                    current_node_id: None,
                    state: json!({
                        "error": error.to_string()
                    }),
                    timeline: Vec::new(),
                    last_signal: None,
                    resume_state: None,
                });
            }
        });

        Ok(running_summary)
    }

    fn publish_summary(&self, summary: &WorkflowRunSummary) {
        let _ = self.store.save_summary(summary);
        let _ = self.events.send(WorkflowRunEvent::from_summary(summary));
    }
}

#[derive(Clone)]
struct StoredWorkflowDefinition {
    id: String,
    workspace_id: String,
    definition: WorkflowDefinition,
}

#[derive(Clone, Default)]
struct RunRegistry {
    state: Arc<Mutex<HashMap<String, String>>>,
}

impl RunRegistry {
    fn bind(&self, run_id: &str, workflow_id: String) {
        if let Ok(mut state) = self.state.lock() {
            state.insert(run_id.to_string(), workflow_id);
        }
    }

    fn resolve(&self, run_id: &str) -> Option<String> {
        self.state.lock().ok()?.get(run_id).cloned()
    }
}

#[derive(Clone, Default)]
struct WorkflowCatalog {
    state: Arc<Mutex<WorkflowCatalogState>>,
}

#[derive(Default)]
struct WorkflowCatalogState {
    workspaces: HashMap<String, WorkspaceRecord>,
    workflows: HashMap<String, StoredWorkflowDefinition>,
}

impl WorkflowCatalog {
    fn new() -> Self {
        let catalog = Self::default();
        catalog.ensure_workspace(Some("default".to_string()), Some("Default".to_string()));
        catalog
    }

    fn ensure_workspace(
        &self,
        workspace_id: Option<String>,
        workspace_name: Option<String>,
    ) -> WorkspaceRecord {
        let id = workspace_id.unwrap_or_else(|| "default".to_string());
        let fallback_name = id.clone();
        let name = workspace_name.unwrap_or(fallback_name);

        let mut state = self.state.lock().expect("workflow catalog lock poisoned");
        state
            .workspaces
            .entry(id.clone())
            .or_insert_with(|| WorkspaceRecord {
                id: id.clone(),
                name,
            })
            .clone()
    }

    fn register_workflow(
        &self,
        workspace_id: &str,
        definition: WorkflowDefinition,
    ) -> WorkflowRecord {
        let workflow_id = new_workflow_id();
        let record = StoredWorkflowDefinition {
            id: workflow_id.clone(),
            workspace_id: workspace_id.to_string(),
            definition,
        };
        let workflow = WorkflowRecord {
            id: record.id.clone(),
            workspace_id: record.workspace_id.clone(),
            workflow_key: record.definition.meta.key.clone(),
            workflow_version: record.definition.meta.version,
            name: record.definition.meta.name.clone(),
        };

        let mut state = self.state.lock().expect("workflow catalog lock poisoned");
        state.workflows.insert(workflow_id, record);
        workflow
    }

    fn get_workflow(&self, workflow_id: &str) -> Option<WorkflowRecord> {
        let state = self.state.lock().ok()?;
        let workflow = state.workflows.get(workflow_id)?;
        Some(WorkflowRecord {
            id: workflow.id.clone(),
            workspace_id: workflow.workspace_id.clone(),
            workflow_key: workflow.definition.meta.key.clone(),
            workflow_version: workflow.definition.meta.version,
            name: workflow.definition.meta.name.clone(),
        })
    }

    fn get_definition(&self, workflow_id: &str) -> Option<StoredWorkflowDefinition> {
        self.state.lock().ok()?.workflows.get(workflow_id).cloned()
    }
}

struct BroadcastRunObserver {
    store: Arc<dyn WorkflowRunStore>,
    events: broadcast::Sender<WorkflowRunEvent>,
}

impl WorkflowRunObserver for BroadcastRunObserver {
    fn on_summary(&self, summary: &WorkflowRunSummary) {
        let _ = self.store.save_summary(summary);
        let _ = self.events.send(WorkflowRunEvent::from_summary(summary));
    }
}

fn new_workflow_id() -> String {
    static WORKFLOW_COUNTER: AtomicU64 = AtomicU64::new(1);
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = WORKFLOW_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("wf-{epoch_ms}-{sequence}")
}
