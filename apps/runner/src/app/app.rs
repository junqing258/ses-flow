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
    }
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
