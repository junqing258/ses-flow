use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::core::definition::WorkflowDefinition;
use crate::core::engine::{WorkflowEngine, new_run_id};
use crate::core::runtime::{
    RunEnvironment, WorkflowRunController, WorkflowRunEvent, WorkflowRunObserver,
    WorkflowRunStatus, WorkflowRunSummary,
};
use crate::error::RunnerError;
use crate::services::WorkflowServices;
use crate::store::{
    InMemoryCatalogStore, InMemoryRunStore, StoredWorkflowDefinition, WorkflowCatalogStore,
    WorkflowDetailRecord, WorkflowRunStore, WorkflowRunner, WorkflowSummaryRecord, WorkspaceRecord,
};

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
    catalog: Arc<dyn WorkflowCatalogStore>,
    run_registry: RunRegistry,
    events: broadcast::Sender<WorkflowRunEvent>,
}

impl WorkflowServer {
    pub fn new() -> Self {
        debug!("initializing workflow server with in-memory catalog");
        let catalog: Arc<dyn WorkflowCatalogStore> = Arc::new(InMemoryCatalogStore::new());
        Self::with_store_and_catalog(Arc::new(InMemoryRunStore::new()), catalog)
    }

    pub fn with_store(store: Arc<dyn WorkflowRunStore>) -> Self {
        debug!("initializing workflow server with in-memory catalog");
        let catalog: Arc<dyn WorkflowCatalogStore> = Arc::new(InMemoryCatalogStore::new());
        Self::with_store_and_catalog(store, catalog)
    }

    pub fn with_catalog(catalog: Arc<dyn WorkflowCatalogStore>) -> Self {
        debug!("initializing workflow server with custom catalog");
        Self::with_store_and_catalog(Arc::new(InMemoryRunStore::new()), catalog)
    }

    pub fn with_store_and_catalog(
        store: Arc<dyn WorkflowRunStore>,
        catalog: Arc<dyn WorkflowCatalogStore>,
    ) -> Self {
        debug!("initializing workflow server with custom store and catalog");
        let (events, _) = broadcast::channel(256);
        let run_registry = RunRegistry::default();
        let observer = Arc::new(BroadcastRunObserver {
            store: store.clone(),
            events: events.clone(),
            run_registry: run_registry.clone(),
        });
        let controller = Arc::new(ServerRunController {
            run_registry: run_registry.clone(),
        });
        let runner = Arc::new(WorkflowRunner::new(
            WorkflowEngine::with_services_observer_and_controller(
                WorkflowServices::with_defaults(),
                observer,
                controller,
            ),
            store.clone(),
        ));

        Self {
            store,
            runner,
            catalog,
            run_registry,
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
        workflow_id: Option<String>,
        definition: WorkflowDefinition,
        editor_document: Option<serde_json::Value>,
    ) -> Result<WorkflowRegistration, ServerError> {
        let requested_workspace_id = workspace_id.as_deref().unwrap_or("default");
        info!(
            workspace_id = requested_workspace_id,
            workflow_key = definition.meta.key,
            workflow_version = definition.meta.version,
            "validating workflow registration",
        );
        definition.validate()?;

        // Ensure workspace exists
        let workspace_record = WorkspaceRecord {
            id: requested_workspace_id.to_string(),
            name: workspace_name.unwrap_or_else(|| requested_workspace_id.to_string()),
        };
        self.catalog.save_workspace(&workspace_record)?;

        let workflow_id = workflow_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(new_workflow_id);
        let existing_workflow = self.catalog.load_workflow(&workflow_id)?;
        let now = Utc::now();
        let stored_workflow = StoredWorkflowDefinition {
            id: workflow_id.clone(),
            workspace_id: workspace_record.id.clone(),
            definition,
            editor_document,
            created_at: existing_workflow
                .as_ref()
                .map(|workflow| workflow.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        self.catalog.save_workflow(&stored_workflow)?;

        info!(
            workflow_id = %workflow_id,
            workspace_id = %workspace_record.id,
            workflow_key = %stored_workflow.definition.meta.key,
            workflow_version = stored_workflow.definition.meta.version,
            "workflow registration completed",
        );

        Ok(WorkflowRegistration {
            workspace_id: stored_workflow.workspace_id,
            workflow_id: stored_workflow.id,
            workflow_key: stored_workflow.definition.meta.key,
            workflow_version: stored_workflow.definition.meta.version,
        })
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowSummaryRecord>, ServerError> {
        let mut workflows = self
            .catalog
            .load_all_workflows()?
            .into_iter()
            .map(|workflow| self.to_workflow_summary(workflow))
            .collect::<Result<Vec<_>, _>>()?;

        workflows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(workflows)
    }

    pub fn get_workflow(&self, workflow_id: &str) -> Result<WorkflowDetailRecord, ServerError> {
        let stored_workflow = self
            .catalog
            .load_workflow(workflow_id)?
            .ok_or_else(|| ServerError::NotFound(format!("workflow not found: {workflow_id}")))?;

        Ok(WorkflowDetailRecord {
            summary: self.to_workflow_summary(stored_workflow.clone())?,
            document: stored_workflow.editor_document,
            workflow: stored_workflow.definition,
        })
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
        info!(workflow_id = %workflow_id, "preparing workflow run");
        let stored_workflow = self
            .catalog
            .load_workflow(workflow_id)?
            .ok_or_else(|| ServerError::NotFound(format!("workflow not found: {workflow_id}")))?;
        let run_id = new_run_id();
        let start_node = stored_workflow.definition.start_node()?.id.clone();
        info!(
            workflow_id = %stored_workflow.id,
            run_id = %run_id,
            start_node_id = %start_node,
            workflow_key = stored_workflow.definition.meta.key,
            workflow_version = stored_workflow.definition.meta.version,
            "workflow run queued",
        );

        self.run_registry.bind(&run_id, stored_workflow.id.clone());

        let summary = WorkflowRunSummary {
            run_id: run_id.clone(),
            workflow_key: stored_workflow.definition.meta.key.clone(),
            workflow_version: stored_workflow.definition.meta.version,
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
            let run_result =
                runner.run_with_id(&stored_workflow.definition, run_id.clone(), trigger, env);
            if let Err(error) = run_result {
                error!(
                    workflow_id = %stored_workflow.id,
                    run_id = %run_id,
                    error = %error,
                    "workflow run failed in background task",
                );
                fallback.publish_summary(&WorkflowRunSummary {
                    run_id: run_id.clone(),
                    workflow_key: stored_workflow.definition.meta.key.clone(),
                    workflow_version: stored_workflow.definition.meta.version,
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
        info!(run_id = %run_id, "preparing workflow resume");
        let workflow_id = self
            .run_registry
            .resolve(run_id)
            .ok_or_else(|| ServerError::NotFound(format!("workflow run not found: {run_id}")))?;
        let stored_workflow = self
            .catalog
            .load_workflow(&workflow_id)?
            .ok_or_else(|| ServerError::NotFound(format!("workflow not found: {workflow_id}")))?;

        let running_summary = WorkflowRunSummary {
            run_id: run_id.to_string(),
            workflow_key: stored_workflow.definition.meta.key.clone(),
            workflow_version: stored_workflow.definition.meta.version,
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
            let resume_result =
                runner.resume_by_run_id(&stored_workflow.definition, &run_id, event);
            if let Err(error) = resume_result {
                error!(
                    workflow_id = %stored_workflow.id,
                    run_id = %run_id,
                    error = %error,
                    "workflow resume failed in background task",
                );
                fallback.publish_summary(&WorkflowRunSummary {
                    run_id: run_id.clone(),
                    workflow_key: stored_workflow.definition.meta.key.clone(),
                    workflow_version: stored_workflow.definition.meta.version,
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

    pub fn terminate_workflow(&self, run_id: &str) -> Result<WorkflowRunSummary, ServerError> {
        let summary = self
            .store
            .load_summary(run_id)?
            .ok_or_else(|| ServerError::NotFound(format!("workflow run not found: {run_id}")))?;

        match summary.status {
            WorkflowRunStatus::Completed
            | WorkflowRunStatus::Failed
            | WorkflowRunStatus::Terminated => Ok(summary),
            WorkflowRunStatus::Waiting => {
                info!(run_id = %run_id, "terminating waiting workflow run");
                let terminated = WorkflowRunSummary {
                    run_id: summary.run_id,
                    workflow_key: summary.workflow_key,
                    workflow_version: summary.workflow_version,
                    status: WorkflowRunStatus::Terminated,
                    current_node_id: summary.current_node_id,
                    state: summary.state,
                    timeline: summary.timeline,
                    last_signal: summary.last_signal,
                    resume_state: None,
                };
                self.publish_summary(&terminated);
                Ok(terminated)
            }
            WorkflowRunStatus::Running => {
                info!(run_id = %run_id, "termination requested for running workflow run");
                self.run_registry.request_termination(run_id);
                Ok(summary)
            }
        }
    }

    fn publish_summary(&self, summary: &WorkflowRunSummary) {
        persist_summary(self.store.as_ref(), summary);
        if is_terminal_status(&summary.status) {
            self.run_registry.finish(&summary.run_id);
        }
        if let Err(error) = self.events.send(WorkflowRunEvent::from_summary(summary)) {
            debug!(
                run_id = %error.0.run_id,
                "skipped workflow summary broadcast because there are no subscribers",
            );
        }
    }

    fn to_workflow_summary(
        &self,
        workflow: StoredWorkflowDefinition,
    ) -> Result<WorkflowSummaryRecord, ServerError> {
        let workspace_name = self
            .catalog
            .load_workspace(&workflow.workspace_id)?
            .map(|workspace| workspace.name);
        let status = workflow
            .definition
            .meta
            .status
            .clone()
            .unwrap_or_else(|| "draft".to_string());
        let normalized_status = if status.eq_ignore_ascii_case("published") {
            "published".to_string()
        } else {
            "draft".to_string()
        };

        Ok(WorkflowSummaryRecord {
            workflow_id: workflow.id,
            workspace_id: workflow.workspace_id,
            workflow_key: workflow.definition.meta.key.clone(),
            workflow_version: workflow.definition.meta.version,
            name: workflow
                .definition
                .meta
                .name
                .clone()
                .unwrap_or_else(|| workflow.definition.meta.key.clone()),
            status: normalized_status.clone(),
            version: format!("v{}", workflow.definition.meta.version),
            owner_name: workspace_name,
            created_at: workflow.created_at,
            updated_at: workflow.updated_at,
            published_at: if normalized_status == "published" {
                Some(workflow.updated_at)
            } else {
                None
            },
        })
    }
}

#[derive(Clone, Default)]
struct RunRegistry {
    state: Arc<Mutex<RunRegistryState>>,
}

#[derive(Default)]
struct RunRegistryState {
    workflow_ids: HashMap<String, String>,
    termination_requests: HashSet<String>,
}

impl RunRegistry {
    fn bind(&self, run_id: &str, workflow_id: String) {
        if let Ok(mut state) = self.state.lock() {
            state.workflow_ids.insert(run_id.to_string(), workflow_id);
            state.termination_requests.remove(run_id);
        } else {
            warn!(run_id = %run_id, "failed to acquire run registry lock while binding run");
        }
    }

    fn resolve(&self, run_id: &str) -> Option<String> {
        self.state.lock().ok()?.workflow_ids.get(run_id).cloned()
    }

    fn request_termination(&self, run_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.termination_requests.insert(run_id.to_string());
        } else {
            warn!(run_id = %run_id, "failed to acquire run registry lock while requesting termination");
        }
    }

    fn should_terminate(&self, run_id: &str) -> bool {
        self.state
            .lock()
            .ok()
            .map(|state| state.termination_requests.contains(run_id))
            .unwrap_or(false)
    }

    fn finish(&self, run_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.workflow_ids.remove(run_id);
            state.termination_requests.remove(run_id);
        } else {
            warn!(run_id = %run_id, "failed to acquire run registry lock while finalizing run");
        }
    }
}

struct ServerRunController {
    run_registry: RunRegistry,
}

impl WorkflowRunController for ServerRunController {
    fn should_terminate(&self, run_id: &str) -> bool {
        self.run_registry.should_terminate(run_id)
    }
}

struct BroadcastRunObserver {
    store: Arc<dyn WorkflowRunStore>,
    events: broadcast::Sender<WorkflowRunEvent>,
    run_registry: RunRegistry,
}

impl WorkflowRunObserver for BroadcastRunObserver {
    fn on_summary(&self, summary: &WorkflowRunSummary) {
        persist_summary(self.store.as_ref(), summary);
        if is_terminal_status(&summary.status) {
            self.run_registry.finish(&summary.run_id);
        }
        if let Err(error) = self.events.send(WorkflowRunEvent::from_summary(summary)) {
            debug!(
                run_id = %error.0.run_id,
                "skipped summary broadcast because there are no subscribers",
            );
        }
    }
}

fn is_terminal_status(status: &WorkflowRunStatus) -> bool {
    matches!(
        status,
        WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Terminated
    )
}

fn persist_summary(store: &dyn WorkflowRunStore, summary: &WorkflowRunSummary) {
    let persistence_result = if is_terminal_status(&summary.status) {
        store.mark_completed(summary)
    } else {
        store.save_summary(summary)
    };

    if let Err(error) = persistence_result {
        warn!(run_id = %summary.run_id, error = %error, "failed to persist workflow summary");
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
