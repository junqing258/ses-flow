use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::RunnerError;
use crate::runtime::{WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary};

pub trait WorkflowRunStore: Send + Sync {
    fn save_summary(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError>;
    fn save_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError>;
    fn load_snapshot(&self, run_id: &str) -> Result<Option<WorkflowRunSnapshot>, RunnerError>;
    fn load_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, RunnerError>;
    fn mark_completed(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError>;
}

#[derive(Clone, Default)]
pub struct InMemoryRunStore {
    state: Arc<Mutex<StoreState>>,
}

#[derive(Default)]
struct StoreState {
    summaries: HashMap<String, WorkflowRunSummary>,
    waiting_snapshots: HashMap<String, WorkflowRunSnapshot>,
}

impl InMemoryRunStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl WorkflowRunStore for InMemoryRunStore {
    fn save_summary(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        state
            .summaries
            .insert(summary.run_id.clone(), summary.clone());
        Ok(())
    }

    fn save_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        state
            .waiting_snapshots
            .insert(snapshot.run_id.clone(), snapshot);
        Ok(())
    }

    fn load_snapshot(&self, run_id: &str) -> Result<Option<WorkflowRunSnapshot>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        Ok(state.waiting_snapshots.get(run_id).cloned())
    }

    fn load_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        Ok(state.summaries.get(run_id).cloned())
    }

    fn mark_completed(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        state
            .summaries
            .insert(summary.run_id.clone(), summary.clone());
        if matches!(
            summary.status,
            WorkflowRunStatus::Completed | WorkflowRunStatus::Failed
        ) {
            state.waiting_snapshots.remove(&summary.run_id);
        }
        Ok(())
    }
}

pub struct WorkflowRunner {
    engine: crate::engine::WorkflowEngine,
    store: Arc<dyn WorkflowRunStore>,
}

impl WorkflowRunner {
    pub fn new(engine: crate::engine::WorkflowEngine, store: Arc<dyn WorkflowRunStore>) -> Self {
        Self { engine, store }
    }

    pub fn run(
        &self,
        definition: &crate::definition::WorkflowDefinition,
        trigger: serde_json::Value,
        env: crate::runtime::RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let summary = self.engine.run(definition, trigger, env)?;
        self.store_summary_and_snapshot(&summary)?;
        Ok(summary)
    }

    pub fn run_with_id(
        &self,
        definition: &crate::definition::WorkflowDefinition,
        run_id: String,
        trigger: serde_json::Value,
        env: crate::runtime::RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let summary = self.engine.run_with_id(definition, run_id, trigger, env)?;
        self.store_summary_and_snapshot(&summary)?;
        Ok(summary)
    }

    pub fn resume_by_run_id(
        &self,
        definition: &crate::definition::WorkflowDefinition,
        run_id: &str,
        resume_input: serde_json::Value,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let snapshot = self
            .store
            .load_snapshot(run_id)?
            .ok_or_else(|| RunnerError::MissingRunSnapshot(run_id.to_string()))?;
        let summary = self.engine.resume(definition, snapshot, resume_input)?;
        self.store_summary_and_snapshot(&summary)?;
        Ok(summary)
    }

    pub fn load_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, RunnerError> {
        self.store.load_summary(run_id)
    }

    pub fn seed_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError> {
        self.store.save_snapshot(snapshot.clone())?;
        self.store.save_summary(&WorkflowRunSummary {
            run_id: snapshot.run_id.clone(),
            workflow_key: snapshot.workflow_key.clone(),
            workflow_version: snapshot.workflow_version,
            status: crate::runtime::WorkflowRunStatus::Waiting,
            current_node_id: Some(snapshot.current_node_id.clone()),
            state: snapshot.state.clone(),
            timeline: snapshot.timeline.clone(),
            last_signal: snapshot.last_signal.clone(),
            resume_state: Some(snapshot),
        })?;
        Ok(())
    }

    fn store_summary_and_snapshot(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        self.store.save_summary(summary)?;

        if let Some(snapshot) = summary.resume_state.clone() {
            self.store.save_snapshot(snapshot)?;
        } else {
            self.store.mark_completed(summary)?;
        }

        Ok(())
    }
}
