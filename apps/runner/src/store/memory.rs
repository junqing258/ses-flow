use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::RunnerError;
use crate::core::runtime::{WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary};

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
