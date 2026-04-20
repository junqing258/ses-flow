use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, error, info, warn};

use super::{WorkflowEventStream, WorkflowEventStreams, WorkflowRunner};
use crate::core::definition::WorkflowDefinition;
use crate::core::engine::{WorkflowEngine, new_run_id};
use crate::core::runtime::{
    RunEnvironment, WorkflowRunController, WorkflowRunObserver, WorkflowRunStatus, WorkflowRunSummary,
};
use crate::error::RunnerError;
use crate::services::WorkflowServices;
use crate::store::{
    InMemoryCatalogStore, InMemoryEditSessionStore, InMemoryRunStore, StoredWorkflowDefinition, WorkflowCatalogStore,
    WorkflowDetailRecord, WorkflowEditSessionRecord, WorkflowEditSessionStore, WorkflowRunRecord, WorkflowRunStore,
    WorkflowSummaryRecord, WorkspaceRecord,
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditSessionDraftOperation {
    RemoveNodeCascade {
        #[serde(rename = "nodeId")]
        node_id: String,
    },
    UpdateNodeConfig {
        #[serde(rename = "nodeId")]
        node_id: String,
        config: Value,
    },
    AddEdge {
        source: String,
        target: String,
        #[serde(rename = "sourceHandle", default)]
        source_handle: Option<String>,
        #[serde(rename = "targetHandle", default)]
        target_handle: Option<String>,
    },
    RemoveEdge {
        #[serde(rename = "edgeId")]
        edge_id: String,
    },
    UpdateEdge {
        #[serde(rename = "edgeId")]
        edge_id: String,
        updates: Value,
    },
}

#[derive(Clone)]
pub struct WorkflowApp {
    store: Arc<dyn WorkflowRunStore>,
    catalog: Arc<dyn WorkflowCatalogStore>,
    edit_sessions: Arc<dyn WorkflowEditSessionStore>,
    run_registry: RunRegistry,
    events: WorkflowEventStreams,
}

impl WorkflowApp {
    pub fn new() -> Self {
        // 默认构造方式主要面向测试和轻量本地使用，因此这里会装配内存版
        // catalog。生产环境装配会显式注入 PostgresCatalogStore。
        debug!("initializing workflow server with in-memory catalog");
        let catalog: Arc<dyn WorkflowCatalogStore> = Arc::new(InMemoryCatalogStore::new());
        let edit_sessions: Arc<dyn WorkflowEditSessionStore> = Arc::new(InMemoryEditSessionStore::new());
        Self::with_store_catalog_and_sessions(Arc::new(InMemoryRunStore::new()), catalog, edit_sessions)
    }

    pub fn with_store(store: Arc<dyn WorkflowRunStore>) -> Self {
        // 这个构造函数会继续使用内存版 catalog，但允许调用方单独替换
        // run store，适合做更聚焦的测试。
        debug!("initializing workflow server with in-memory catalog");
        let catalog: Arc<dyn WorkflowCatalogStore> = Arc::new(InMemoryCatalogStore::new());
        let edit_sessions: Arc<dyn WorkflowEditSessionStore> = Arc::new(InMemoryEditSessionStore::new());
        Self::with_store_catalog_and_sessions(store, catalog, edit_sessions)
    }

    pub fn with_catalog(catalog: Arc<dyn WorkflowCatalogStore>) -> Self {
        debug!("initializing workflow server with custom catalog");
        let edit_sessions: Arc<dyn WorkflowEditSessionStore> = Arc::new(InMemoryEditSessionStore::new());
        Self::with_store_catalog_and_sessions(Arc::new(InMemoryRunStore::new()), catalog, edit_sessions)
    }

    pub fn with_store_and_catalog(store: Arc<dyn WorkflowRunStore>, catalog: Arc<dyn WorkflowCatalogStore>) -> Self {
        let edit_sessions: Arc<dyn WorkflowEditSessionStore> = Arc::new(InMemoryEditSessionStore::new());
        Self::with_store_catalog_and_sessions(store, catalog, edit_sessions)
    }

    pub fn with_store_catalog_and_sessions(
        store: Arc<dyn WorkflowRunStore>,
        catalog: Arc<dyn WorkflowCatalogStore>,
        edit_sessions: Arc<dyn WorkflowEditSessionStore>,
    ) -> Self {
        debug!("initializing workflow server with custom store and catalog");
        let run_registry = RunRegistry::default();
        let events = WorkflowEventStreams::default();

        Self {
            store,
            catalog,
            edit_sessions,
            run_registry,
            events,
        }
    }

    fn build_workflow_services(&self) -> Result<WorkflowServices, AppError> {
        let mut services = WorkflowServices::with_defaults();

        for workflow in self.catalog.load_all_workflows()? {
            services
                .workflow_definitions
                .register(workflow.definition.meta.key.clone(), workflow.definition.clone());
            services.workflow_definitions.register(workflow.id, workflow.definition);
        }

        Ok(services)
    }

    fn build_runner(&self) -> Result<WorkflowRunner, AppError> {
        let observer = Arc::new(PersistingRunObserver {
            store: self.store.clone(),
            run_registry: self.run_registry.clone(),
            events: self.events.clone(),
        });
        let controller = Arc::new(WorkflowAppRunController {
            run_registry: self.run_registry.clone(),
        });

        Ok(WorkflowRunner::new(
            WorkflowEngine::with_services_observer_and_controller(
                self.build_workflow_services()?,
                observer,
                controller,
            ),
            self.store.clone(),
        ))
    }

    pub fn register_workflow(
        &self,
        workspace_id: Option<String>,
        workspace_name: Option<String>,
        workflow_id: Option<String>,
        definition: WorkflowDefinition,
        editor_document: Option<serde_json::Value>,
    ) -> Result<WorkflowRegistration, AppError> {
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
        self.events.publish_workflow_changed(&stored_workflow.id);

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

    pub fn create_edit_session(
        &self,
        workspace_id: Option<String>,
        workflow_id: Option<String>,
        definition: WorkflowDefinition,
        editor_document: Option<serde_json::Value>,
    ) -> Result<WorkflowEditSessionRecord, AppError> {
        definition.validate()?;

        let now = Utc::now();
        let session = WorkflowEditSessionRecord {
            session_id: new_session_id(),
            workspace_id: workspace_id.unwrap_or_else(|| "ses-workflow-editor".to_string()),
            workflow_id,
            workflow: definition,
            editor_document,
            created_at: now,
            updated_at: now,
        };

        self.edit_sessions.save_session(&session)?;
        self.events.publish_session_changed(&session);
        Ok(session)
    }

    pub fn get_edit_session(&self, session_id: &str) -> Result<WorkflowEditSessionRecord, AppError> {
        self.edit_sessions
            .load_session(session_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow edit session not found: {session_id}")))
    }

    pub fn update_edit_session(
        &self,
        session_id: &str,
        workflow_id: Option<String>,
        definition: WorkflowDefinition,
        editor_document: Option<serde_json::Value>,
    ) -> Result<WorkflowEditSessionRecord, AppError> {
        definition.validate()?;
        let existing = self
            .edit_sessions
            .load_session(session_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow edit session not found: {session_id}")))?;

        let session = WorkflowEditSessionRecord {
            session_id: existing.session_id,
            workspace_id: existing.workspace_id,
            workflow_id: workflow_id.or(existing.workflow_id),
            workflow: definition,
            editor_document,
            created_at: existing.created_at,
            updated_at: Utc::now(),
        };

        self.edit_sessions.save_session(&session)?;
        self.events.publish_session_changed(&session);
        Ok(session)
    }

    pub fn apply_edit_session_operations(
        &self,
        session_id: &str,
        workflow_id: Option<String>,
        operations: Vec<EditSessionDraftOperation>,
    ) -> Result<WorkflowEditSessionRecord, AppError> {
        if operations.is_empty() {
            return Err(AppError::BadRequest(
                "edit session operations cannot be empty".to_string(),
            ));
        }

        let existing = self
            .edit_sessions
            .load_session(session_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow edit session not found: {session_id}")))?;
        let mut workflow = existing.workflow.clone();
        let mut editor_document = existing.editor_document.clone();

        for operation in &operations {
            apply_edit_session_operation(&mut workflow, &mut editor_document, operation)?;
        }

        workflow.validate()?;

        let session = WorkflowEditSessionRecord {
            session_id: existing.session_id,
            workspace_id: existing.workspace_id,
            workflow_id: workflow_id.or(existing.workflow_id),
            workflow,
            editor_document,
            created_at: existing.created_at,
            updated_at: Utc::now(),
        };

        self.edit_sessions.save_session(&session)?;
        self.events.publish_session_changed(&session);
        Ok(session)
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowSummaryRecord>, AppError> {
        let mut workflows = self
            .catalog
            .load_all_workflows()?
            .into_iter()
            .map(|workflow| {
                let active_run_count = self
                    .list_active_runs(&workflow.definition.meta.key, workflow.definition.meta.version)?
                    .len() as u32;
                self.to_workflow_summary(workflow, active_run_count)
            })
            .collect::<Result<Vec<_>, _>>()?;

        workflows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(workflows)
    }

    pub fn get_workflow(&self, workflow_id: &str) -> Result<WorkflowDetailRecord, AppError> {
        let stored_workflow = self
            .catalog
            .load_workflow(workflow_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow not found: {workflow_id}")))?;
        let active_run_count = self
            .list_active_runs(
                &stored_workflow.definition.meta.key,
                stored_workflow.definition.meta.version,
            )?
            .len() as u32;

        Ok(WorkflowDetailRecord {
            summary: self.to_workflow_summary(stored_workflow.clone(), active_run_count)?,
            document: stored_workflow.editor_document,
            workflow: stored_workflow.definition,
        })
    }

    pub fn list_workflow_runs(&self, workflow_id: &str) -> Result<Vec<WorkflowRunRecord>, AppError> {
        let stored_workflow = self
            .catalog
            .load_workflow(workflow_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow not found: {workflow_id}")))?;

        self.list_active_runs(
            &stored_workflow.definition.meta.key,
            stored_workflow.definition.meta.version,
        )
    }

    pub fn refresh_catalog(&self) -> Result<(), AppError> {
        self.catalog.refresh()?;
        Ok(())
    }

    pub fn get_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, AppError> {
        Ok(self.store.load_summary(run_id)?)
    }

    pub fn subscribe_run_events(&self, run_id: &str) -> WorkflowEventStream {
        self.events.subscribe_run(run_id)
    }

    pub fn subscribe_edit_session_events(&self, session_id: &str) -> WorkflowEventStream {
        self.events.subscribe_session(session_id)
    }

    pub fn subscribe_workflow_events(&self, workflow_id: &str) -> WorkflowEventStream {
        self.events.subscribe_workflow(workflow_id)
    }

    pub fn subscribe_workflows_events(&self) -> WorkflowEventStream {
        self.events.subscribe_workflows()
    }

    pub async fn start_workflow(
        &self,
        workflow_id: &str,
        trigger: serde_json::Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, AppError> {
        info!(workflow_id = %workflow_id, "preparing workflow run");
        let stored_workflow = self
            .catalog
            .load_workflow(workflow_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow not found: {workflow_id}")))?;
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

        let runner = self.build_runner()?;
        let fallback = self.clone();
        tokio::task::spawn_blocking(move || {
            let run_result = runner.run_with_id(&stored_workflow.definition, run_id.clone(), trigger, env);
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
    ) -> Result<WorkflowRunSummary, AppError> {
        info!(run_id = %run_id, "preparing workflow resume");
        let workflow_id = self
            .run_registry
            .resolve(run_id)
            .ok_or_else(|| AppError::NotFound(format!("workflow run not found: {run_id}")))?;
        let stored_workflow = self
            .catalog
            .load_workflow(&workflow_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow not found: {workflow_id}")))?;

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

        let runner = self.build_runner()?;
        let fallback = self.clone();
        let run_id = run_id.to_string();
        tokio::task::spawn_blocking(move || {
            let resume_result = runner.resume_by_run_id(&stored_workflow.definition, &run_id, event);
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

    pub fn terminate_workflow(&self, run_id: &str) -> Result<WorkflowRunSummary, AppError> {
        let summary = self
            .store
            .load_summary(run_id)?
            .ok_or_else(|| AppError::NotFound(format!("workflow run not found: {run_id}")))?;

        match summary.status {
            WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Terminated => Ok(summary),
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
                self.publish_summary_with_workflow_fallback(&terminated);
                Ok(terminated)
            }
            WorkflowRunStatus::Running => {
                if let Some(workflow_id) = self.run_registry.resolve(run_id) {
                    info!(run_id = %run_id, "termination requested for running workflow run");
                    self.run_registry.request_termination(run_id);
                    self.events.publish_workflow_runs_changed(&workflow_id, &summary);
                    Ok(summary)
                } else {
                    info!(run_id = %run_id, "terminating orphaned running workflow run");
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
                    self.publish_summary_with_workflow_fallback(&terminated);
                    Ok(terminated)
                }
            }
        }
    }

    fn publish_summary(&self, summary: &WorkflowRunSummary) {
        persist_summary_and_publish_events(self.store.as_ref(), &self.run_registry, &self.events, summary);
    }

    fn publish_summary_with_workflow_fallback(&self, summary: &WorkflowRunSummary) {
        let workflow_id = self.run_registry.resolve(&summary.run_id);
        self.publish_summary(summary);

        if workflow_id.is_none() {
            if let Some(workflow_id) = self.resolve_workflow_id_for_summary(summary) {
                self.events.publish_workflow_runs_changed(&workflow_id, summary);
            }
        }
    }

    fn resolve_workflow_id_for_summary(&self, summary: &WorkflowRunSummary) -> Option<String> {
        match self.catalog.load_all_workflows() {
            Ok(workflows) => workflows.into_iter().find_map(|workflow| {
                (workflow.definition.meta.key == summary.workflow_key
                    && workflow.definition.meta.version == summary.workflow_version)
                    .then_some(workflow.id)
            }),
            Err(error) => {
                warn!(
                    run_id = %summary.run_id,
                    workflow_key = %summary.workflow_key,
                    workflow_version = summary.workflow_version,
                    error = %error,
                    "failed to resolve workflow id for workflow run summary",
                );
                None
            }
        }
    }

    fn to_workflow_summary(
        &self,
        workflow: StoredWorkflowDefinition,
        running_run_count: u32,
    ) -> Result<WorkflowSummaryRecord, AppError> {
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
            running_run_count,
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

    fn list_active_runs(&self, workflow_key: &str, workflow_version: u32) -> Result<Vec<WorkflowRunRecord>, AppError> {
        Ok(self
            .store
            .list_runs(workflow_key, workflow_version)?
            .into_iter()
            .filter(|run| is_active_run_status(&run.status) && !self.run_registry.should_terminate(&run.run_id))
            .collect())
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

struct WorkflowAppRunController {
    run_registry: RunRegistry,
}

impl WorkflowRunController for WorkflowAppRunController {
    fn should_terminate(&self, run_id: &str) -> bool {
        self.run_registry.should_terminate(run_id)
    }
}

struct PersistingRunObserver {
    store: Arc<dyn WorkflowRunStore>,
    run_registry: RunRegistry,
    events: WorkflowEventStreams,
}

impl WorkflowRunObserver for PersistingRunObserver {
    fn on_summary(&self, summary: &WorkflowRunSummary) {
        persist_summary_and_publish_events(self.store.as_ref(), &self.run_registry, &self.events, summary);
    }
}

fn is_terminal_status(status: &WorkflowRunStatus) -> bool {
    matches!(
        status,
        WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Terminated
    )
}

fn is_active_run_status(status: &WorkflowRunStatus) -> bool {
    matches!(status, WorkflowRunStatus::Running | WorkflowRunStatus::Waiting)
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

fn persist_summary_and_publish_events(
    store: &dyn WorkflowRunStore,
    run_registry: &RunRegistry,
    events: &WorkflowEventStreams,
    summary: &WorkflowRunSummary,
) {
    let workflow_id = run_registry.resolve(&summary.run_id);

    persist_summary(store, summary);
    events.publish_run_changed(summary, workflow_id.as_deref());

    if let Some(workflow_id) = workflow_id.as_deref() {
        events.publish_workflow_runs_changed(workflow_id, summary);
    }

    if is_terminal_status(&summary.status) {
        run_registry.finish(&summary.run_id);
    }
}

fn apply_edit_session_operation(
    workflow: &mut WorkflowDefinition,
    editor_document: &mut Option<Value>,
    operation: &EditSessionDraftOperation,
) -> Result<(), AppError> {
    match operation {
        EditSessionDraftOperation::RemoveNodeCascade { node_id } => {
            remove_node_cascade(workflow, editor_document, node_id)
        }
        EditSessionDraftOperation::UpdateNodeConfig { node_id, config } => {
            update_node_config(workflow, node_id, config)
        }
        EditSessionDraftOperation::AddEdge {
            source,
            target,
            source_handle,
            target_handle,
        } => add_edge(
            workflow,
            editor_document,
            source,
            target,
            source_handle.as_deref(),
            target_handle.as_deref(),
        ),
        EditSessionDraftOperation::RemoveEdge { edge_id } => remove_edge(workflow, editor_document, edge_id),
        EditSessionDraftOperation::UpdateEdge { edge_id, updates } => {
            update_edge(workflow, editor_document, edge_id, updates)
        }
    }
}

#[derive(Clone)]
struct EdgeReference {
    edge_id: String,
    source: String,
    target: String,
    source_handle: Option<String>,
    target_handle: Option<String>,
    label: Option<String>,
}

fn remove_node_cascade(
    workflow: &mut WorkflowDefinition,
    editor_document: &mut Option<Value>,
    node_id: &str,
) -> Result<(), AppError> {
    if workflow.node(node_id).is_none() {
        return Err(AppError::BadRequest(format!(
            "node does not exist in edit session workflow: {node_id}"
        )));
    }

    let incoming_transitions = workflow
        .transitions
        .iter()
        .filter(|transition| transition.to == node_id)
        .cloned()
        .collect::<Vec<_>>();
    let outgoing_transitions = workflow
        .transitions
        .iter()
        .filter(|transition| transition.from == node_id)
        .cloned()
        .collect::<Vec<_>>();

    workflow.nodes.retain(|node| node.id != node_id);
    workflow
        .transitions
        .retain(|transition| transition.from != node_id && transition.to != node_id);
    reconnect_workflow_transitions(&mut workflow.transitions, &incoming_transitions, &outgoing_transitions);

    if let Some(document) = editor_document.as_mut() {
        remove_node_from_editor_document(document, node_id);
    }

    Ok(())
}

fn update_node_config(workflow: &mut WorkflowDefinition, node_id: &str, config: &Value) -> Result<(), AppError> {
    let node = workflow
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or_else(|| AppError::BadRequest(format!("node does not exist in edit session workflow: {node_id}")))?;

    let Some(config_object) = config.as_object() else {
        return Err(AppError::BadRequest(
            "update_node_config.config must be a JSON object".to_string(),
        ));
    };

    node.config = Value::Object(config_object.clone());
    Ok(())
}

fn add_edge(
    workflow: &mut WorkflowDefinition,
    editor_document: &mut Option<Value>,
    source: &str,
    target: &str,
    source_handle: Option<&str>,
    target_handle: Option<&str>,
) -> Result<(), AppError> {
    ensure_node_exists(workflow, source)?;
    ensure_node_exists(workflow, target)?;

    let transition = crate::core::definition::TransitionDefinition {
        from: source.to_string(),
        to: target.to_string(),
        condition: None,
        priority: None,
        label: None,
        branch_type: None,
    };

    if !workflow.transitions.iter().any(|existing| {
        existing.from == transition.from
            && existing.to == transition.to
            && existing.condition == transition.condition
            && existing.priority == transition.priority
            && existing.label == transition.label
            && existing.branch_type == transition.branch_type
    }) {
        workflow.transitions.push(transition);
    }

    if let Some(document) = editor_document.as_mut() {
        upsert_editor_edge(
            document,
            &EdgeReference {
                edge_id: build_editor_edge_id(source, target, source_handle, target_handle),
                source: source.to_string(),
                target: target.to_string(),
                source_handle: source_handle.map(str::to_string),
                target_handle: target_handle.map(str::to_string),
                label: None,
            },
        )?;
    }

    Ok(())
}

fn remove_edge(
    workflow: &mut WorkflowDefinition,
    editor_document: &mut Option<Value>,
    edge_id: &str,
) -> Result<(), AppError> {
    let reference = resolve_edge_reference(editor_document.as_ref(), edge_id)?;
    let transition_index = find_transition_index(workflow, &reference)
        .ok_or_else(|| AppError::BadRequest(format!("edge does not exist in edit session workflow: {edge_id}")))?;
    workflow.transitions.remove(transition_index);

    if let Some(document) = editor_document.as_mut() {
        remove_editor_edge(document, edge_id);
    }

    Ok(())
}

fn update_edge(
    workflow: &mut WorkflowDefinition,
    editor_document: &mut Option<Value>,
    edge_id: &str,
    updates: &Value,
) -> Result<(), AppError> {
    let updates = updates
        .as_object()
        .ok_or_else(|| AppError::BadRequest("update_edge.updates must be a JSON object".to_string()))?;
    let current_reference = resolve_edge_reference(editor_document.as_ref(), edge_id)?;
    let transition_index = find_transition_index(workflow, &current_reference)
        .ok_or_else(|| AppError::BadRequest(format!("edge does not exist in edit session workflow: {edge_id}")))?;
    let transition = workflow.transitions[transition_index].clone();

    let next_source = optional_string_update(updates, "source").unwrap_or_else(|| current_reference.source.clone());
    let next_target = optional_string_update(updates, "target").unwrap_or_else(|| current_reference.target.clone());
    ensure_node_exists(workflow, &next_source)?;
    ensure_node_exists(workflow, &next_target)?;

    let next_source_handle =
        optional_nullable_string_update(updates, "sourceHandle").unwrap_or(current_reference.source_handle.clone());
    let next_target_handle =
        optional_nullable_string_update(updates, "targetHandle").unwrap_or(current_reference.target_handle.clone());
    let next_label = optional_nullable_string_update(updates, "label").unwrap_or(current_reference.label.clone());
    let next_condition = optional_nullable_string_update(updates, "condition").unwrap_or(transition.condition.clone());
    let next_branch_type =
        optional_nullable_string_update(updates, "branchType").unwrap_or(transition.branch_type.clone());
    let next_priority = optional_nullable_i32_update(updates, "priority").unwrap_or(transition.priority);

    let next_transition = crate::core::definition::TransitionDefinition {
        from: next_source.clone(),
        to: next_target.clone(),
        condition: next_condition,
        priority: next_priority,
        label: next_label.clone(),
        branch_type: next_branch_type,
    };

    if workflow.transitions.iter().enumerate().any(|(index, existing)| {
        index != transition_index
            && existing.from == next_transition.from
            && existing.to == next_transition.to
            && existing.condition == next_transition.condition
            && existing.priority == next_transition.priority
            && existing.label == next_transition.label
            && existing.branch_type == next_transition.branch_type
    }) {
        return Err(AppError::BadRequest(format!(
            "updated edge would duplicate an existing transition: {edge_id}"
        )));
    }

    workflow.transitions[transition_index] = next_transition;

    if let Some(document) = editor_document.as_mut() {
        update_editor_edge(
            document,
            &current_reference,
            EdgeReference {
                edge_id: updates
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        if updates.contains_key("source")
                            || updates.contains_key("target")
                            || updates.contains_key("sourceHandle")
                            || updates.contains_key("targetHandle")
                        {
                            build_editor_edge_id(
                                &next_source,
                                &next_target,
                                next_source_handle.as_deref(),
                                next_target_handle.as_deref(),
                            )
                        } else {
                            current_reference.edge_id.clone()
                        }
                    }),
                source: next_source,
                target: next_target,
                source_handle: next_source_handle,
                target_handle: next_target_handle,
                label: next_label,
            },
            updates,
        )?;
    }

    Ok(())
}

fn reconnect_workflow_transitions(
    transitions: &mut Vec<crate::core::definition::TransitionDefinition>,
    incoming_transitions: &[crate::core::definition::TransitionDefinition],
    outgoing_transitions: &[crate::core::definition::TransitionDefinition],
) {
    if incoming_transitions.len() != 1 || outgoing_transitions.len() != 1 {
        return;
    }

    let incoming = &incoming_transitions[0];
    let outgoing = &outgoing_transitions[0];

    if incoming.from == outgoing.to {
        return;
    }

    let reconnected = crate::core::definition::TransitionDefinition {
        from: incoming.from.clone(),
        to: outgoing.to.clone(),
        condition: incoming.condition.clone(),
        priority: incoming.priority,
        label: incoming.label.clone(),
        branch_type: incoming.branch_type.clone(),
    };

    if transitions.iter().any(|transition| {
        transition.from == reconnected.from
            && transition.to == reconnected.to
            && transition.condition == reconnected.condition
            && transition.priority == reconnected.priority
            && transition.label == reconnected.label
            && transition.branch_type == reconnected.branch_type
    }) {
        return;
    }

    transitions.push(reconnected);
}

fn ensure_node_exists(workflow: &WorkflowDefinition, node_id: &str) -> Result<(), AppError> {
    if workflow.node(node_id).is_none() {
        return Err(AppError::BadRequest(format!(
            "node does not exist in edit session workflow: {node_id}"
        )));
    }

    Ok(())
}

fn resolve_edge_reference(editor_document: Option<&Value>, edge_id: &str) -> Result<EdgeReference, AppError> {
    if let Some(reference) = editor_document.and_then(|document| find_editor_edge(document, edge_id)) {
        return Ok(reference);
    }

    parse_edge_reference_from_id(edge_id).ok_or_else(|| {
        AppError::BadRequest(format!(
            "edge does not exist in edit session editor document: {edge_id}"
        ))
    })
}

fn find_transition_index(workflow: &WorkflowDefinition, reference: &EdgeReference) -> Option<usize> {
    workflow.transitions.iter().position(|transition| {
        transition.from == reference.source
            && transition.to == reference.target
            && match reference.label.as_deref() {
                Some(label) => transition.label.as_deref() == Some(label),
                None => true,
            }
    })
}

fn find_editor_edge(document: &Value, edge_id: &str) -> Option<EdgeReference> {
    document
        .get("graph")
        .and_then(|graph| graph.get("edges"))
        .and_then(Value::as_array)
        .and_then(|edges| {
            edges
                .iter()
                .find(|edge| edge.get("id").and_then(Value::as_str) == Some(edge_id))
                .and_then(extract_edge_reference_from_value)
        })
}

fn extract_edge_reference_from_value(edge: &Value) -> Option<EdgeReference> {
    Some(EdgeReference {
        edge_id: edge.get("id").and_then(Value::as_str)?.to_string(),
        source: edge.get("source").and_then(Value::as_str)?.to_string(),
        target: edge.get("target").and_then(Value::as_str)?.to_string(),
        source_handle: edge.get("sourceHandle").and_then(Value::as_str).map(str::to_string),
        target_handle: edge.get("targetHandle").and_then(Value::as_str).map(str::to_string),
        label: edge.get("label").and_then(Value::as_str).map(str::to_string),
    })
}

fn parse_edge_reference_from_id(edge_id: &str) -> Option<EdgeReference> {
    let body = edge_id.strip_prefix("edge:")?;
    let (source_segment, target_segment) = body.split_once("->")?;
    let (source, source_handle) = split_edge_side(source_segment)?;
    let (target, target_handle) = split_edge_side_with_suffix(target_segment)?;

    Some(EdgeReference {
        edge_id: edge_id.to_string(),
        source,
        target,
        source_handle,
        target_handle,
        label: None,
    })
}

fn split_edge_side(segment: &str) -> Option<(String, Option<String>)> {
    let (node_id, handle) = segment.rsplit_once(':')?;
    Some((node_id.to_string(), normalize_edge_handle(handle)))
}

fn split_edge_side_with_suffix(segment: &str) -> Option<(String, Option<String>)> {
    let mut parts = segment.rsplitn(3, ':').collect::<Vec<_>>();
    parts.reverse();

    match parts.as_slice() {
        [node_id, handle] => Some(((*node_id).to_string(), normalize_edge_handle(handle))),
        [node_id, handle, suffix] if suffix.chars().all(|char| char.is_ascii_digit()) => {
            Some(((*node_id).to_string(), normalize_edge_handle(handle)))
        }
        _ => None,
    }
}

fn normalize_edge_handle(handle: &str) -> Option<String> {
    if handle.is_empty() || handle == "default" {
        None
    } else {
        Some(handle.to_string())
    }
}

fn optional_string_update(updates: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    updates.get(key).and_then(Value::as_str).map(str::to_string)
}

fn optional_nullable_string_update(updates: &serde_json::Map<String, Value>, key: &str) -> Option<Option<String>> {
    updates.get(key).map(|value| match value {
        Value::Null => None,
        Value::String(value) => Some(value.to_string()),
        _ => None,
    })
}

fn optional_nullable_i32_update(updates: &serde_json::Map<String, Value>, key: &str) -> Option<Option<i32>> {
    updates.get(key).map(|value| match value {
        Value::Null => None,
        Value::Number(value) => value.as_i64().and_then(|number| i32::try_from(number).ok()),
        _ => None,
    })
}

fn upsert_editor_edge(document: &mut Value, reference: &EdgeReference) -> Result<(), AppError> {
    let Some(edges) = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("edges"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };

    if edges.iter().any(|edge| {
        edge.get("id").and_then(Value::as_str) == Some(reference.edge_id.as_str())
            || (edge.get("source").and_then(Value::as_str) == Some(reference.source.as_str())
                && edge.get("sourceHandle").and_then(Value::as_str) == reference.source_handle.as_deref()
                && edge.get("target").and_then(Value::as_str) == Some(reference.target.as_str())
                && edge.get("targetHandle").and_then(Value::as_str) == reference.target_handle.as_deref())
    }) {
        return Ok(());
    }

    let mut edge = serde_json::Map::new();
    edge.insert("id".to_string(), Value::String(reference.edge_id.clone()));
    edge.insert("source".to_string(), Value::String(reference.source.clone()));
    edge.insert("target".to_string(), Value::String(reference.target.clone()));
    set_optional_string_field(&mut edge, "sourceHandle", reference.source_handle.as_deref());
    set_optional_string_field(&mut edge, "targetHandle", reference.target_handle.as_deref());
    set_optional_string_field(&mut edge, "label", reference.label.as_deref());

    if edges
        .iter()
        .any(|edge| edge.get("id").and_then(Value::as_str) == Some(reference.edge_id.as_str()))
    {
        return Err(AppError::BadRequest(format!(
            "edge already exists in edit session editor document: {}",
            reference.edge_id
        )));
    }

    edges.push(Value::Object(edge));
    Ok(())
}

fn remove_editor_edge(document: &mut Value, edge_id: &str) {
    if let Some(edges) = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("edges"))
        .and_then(Value::as_array_mut)
    {
        edges.retain(|edge| edge.get("id").and_then(Value::as_str) != Some(edge_id));
    }
}

fn update_editor_edge(
    document: &mut Value,
    current_reference: &EdgeReference,
    next_reference: EdgeReference,
    updates: &serde_json::Map<String, Value>,
) -> Result<(), AppError> {
    let Some(edges) = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("edges"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };

    let edge_index = edges
        .iter()
        .position(|edge| edge.get("id").and_then(Value::as_str) == Some(current_reference.edge_id.as_str()))
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "edge does not exist in edit session editor document: {}",
                current_reference.edge_id
            ))
        })?;

    if edges.iter().enumerate().any(|(index, edge)| {
        index != edge_index && edge.get("id").and_then(Value::as_str) == Some(next_reference.edge_id.as_str())
    }) {
        return Err(AppError::BadRequest(format!(
            "updated edge would duplicate an existing editor edge: {}",
            next_reference.edge_id
        )));
    }

    let Some(existing_edge) = edges[edge_index].as_object().cloned() else {
        return Err(AppError::BadRequest(format!(
            "edge has invalid JSON shape in edit session editor document: {}",
            current_reference.edge_id
        )));
    };

    let mut next_edge = existing_edge;

    for (key, value) in updates {
        if value.is_null() {
            next_edge.remove(key);
        } else {
            next_edge.insert(key.clone(), value.clone());
        }
    }

    next_edge.insert("id".to_string(), Value::String(next_reference.edge_id));
    next_edge.insert("source".to_string(), Value::String(next_reference.source));
    next_edge.insert("target".to_string(), Value::String(next_reference.target));
    set_optional_string_field(&mut next_edge, "sourceHandle", next_reference.source_handle.as_deref());
    set_optional_string_field(&mut next_edge, "targetHandle", next_reference.target_handle.as_deref());
    set_optional_string_field(&mut next_edge, "label", next_reference.label.as_deref());

    edges[edge_index] = Value::Object(next_edge);
    Ok(())
}

fn remove_node_from_editor_document(document: &mut Value, node_id: &str) {
    let mut removed_node_ids = HashSet::from([node_id.to_string()]);
    let mut incoming_edges = Vec::new();
    let mut outgoing_edges = Vec::new();

    if let Some(nodes) = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("nodes"))
        .and_then(Value::as_array_mut)
    {
        for node in nodes.iter() {
            let current_id = node.get("id").and_then(Value::as_str);
            let parent_node = node.get("parentNode").and_then(Value::as_str);

            if current_id == Some(node_id) || parent_node == Some(node_id) {
                if let Some(current_id) = current_id {
                    removed_node_ids.insert(current_id.to_string());
                }
            }
        }

        nodes.retain(|node| {
            let Some(current_id) = node.get("id").and_then(Value::as_str) else {
                return true;
            };

            !removed_node_ids.contains(current_id)
        });
    }

    if let Some(edges) = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("edges"))
        .and_then(Value::as_array_mut)
    {
        for edge in edges.iter() {
            let source = edge.get("source").and_then(Value::as_str);
            let target = edge.get("target").and_then(Value::as_str);

            if target == Some(node_id) {
                incoming_edges.push(edge.clone());
            }

            if source == Some(node_id) {
                outgoing_edges.push(edge.clone());
            }
        }

        edges.retain(|edge| {
            let source = edge.get("source").and_then(Value::as_str);
            let target = edge.get("target").and_then(Value::as_str);

            !source.is_some_and(|value| removed_node_ids.contains(value))
                && !target.is_some_and(|value| removed_node_ids.contains(value))
        });

        reconnect_editor_edges(edges, &incoming_edges, &outgoing_edges);
    }

    if let Some(panels) = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("panels"))
        .and_then(Value::as_object_mut)
    {
        panels.retain(|panel_node_id, _| !removed_node_ids.contains(panel_node_id));
    }

    if let Some(editor) = document.get_mut("editor").and_then(Value::as_object_mut) {
        let should_clear_selection = editor
            .get("selectedNodeId")
            .and_then(Value::as_str)
            .is_some_and(|selected_node_id| removed_node_ids.contains(selected_node_id));

        if should_clear_selection {
            editor.remove("selectedNodeId");
        }
    }
}

fn reconnect_editor_edges(edges: &mut Vec<Value>, incoming_edges: &[Value], outgoing_edges: &[Value]) {
    if incoming_edges.len() != 1 || outgoing_edges.len() != 1 {
        return;
    }

    let incoming = &incoming_edges[0];
    let outgoing = &outgoing_edges[0];
    let source = incoming.get("source").and_then(Value::as_str);
    let target = outgoing.get("target").and_then(Value::as_str);

    let (Some(source), Some(target)) = (source, target) else {
        return;
    };

    if source == target {
        return;
    }

    let source_handle = incoming.get("sourceHandle").and_then(Value::as_str);
    let target_handle = outgoing.get("targetHandle").and_then(Value::as_str);

    if edges.iter().any(|edge| {
        edge.get("source").and_then(Value::as_str) == Some(source)
            && edge.get("sourceHandle").and_then(Value::as_str) == source_handle
            && edge.get("target").and_then(Value::as_str) == Some(target)
            && edge.get("targetHandle").and_then(Value::as_str) == target_handle
    }) {
        return;
    }

    let Some(mut reconnected) = incoming.as_object().cloned() else {
        return;
    };

    reconnected.insert("source".to_string(), Value::String(source.to_string()));
    reconnected.insert("target".to_string(), Value::String(target.to_string()));
    set_optional_string_field(&mut reconnected, "sourceHandle", source_handle);
    set_optional_string_field(&mut reconnected, "targetHandle", target_handle);

    let edge_id = format!(
        "edge:{source}:{}->{target}:{}",
        source_handle.unwrap_or("default"),
        target_handle.unwrap_or("default"),
    );
    reconnected.insert("id".to_string(), Value::String(edge_id));

    edges.push(Value::Object(reconnected));
}

fn build_editor_edge_id(
    source: &str,
    target: &str,
    source_handle: Option<&str>,
    target_handle: Option<&str>,
) -> String {
    format!(
        "edge:{source}:{}->{target}:{}",
        source_handle.unwrap_or("default"),
        target_handle.unwrap_or("default"),
    )
}

fn set_optional_string_field(object: &mut serde_json::Map<String, Value>, key: &str, value: Option<&str>) {
    match value {
        Some(value) => {
            object.insert(key.to_string(), Value::String(value.to_string()));
        }
        None => {
            object.remove(key);
        }
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

fn new_session_id() -> String {
    static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("sess-{epoch_ms}-{sequence}")
}
