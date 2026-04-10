use std::sync::Arc;

use crate::core::engine::WorkflowEngine;
use crate::core::definition::WorkflowDefinition;
use crate::core::runtime::{
    WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary, RunEnvironment,
};
use crate::error::RunnerError;

use super::WorkflowRunStore;

pub struct WorkflowRunner {
    engine: WorkflowEngine,
    store: Arc<dyn WorkflowRunStore>,
}

impl WorkflowRunner {
    pub fn new(engine: WorkflowEngine, store: Arc<dyn WorkflowRunStore>) -> Self {
        Self { engine, store }
    }

    pub fn run(
        &self,
        definition: &WorkflowDefinition,
        trigger: serde_json::Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let summary = self.engine.run(definition, trigger, env)?;
        self.store_summary_and_snapshot(&summary)?;
        Ok(summary)
    }

    pub fn run_with_id(
        &self,
        definition: &WorkflowDefinition,
        run_id: String,
        trigger: serde_json::Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let summary = self.engine.run_with_id(definition, run_id, trigger, env)?;
        self.store_summary_and_snapshot(&summary)?;
        Ok(summary)
    }

    pub fn resume_by_run_id(
        &self,
        definition: &WorkflowDefinition,
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
            status: WorkflowRunStatus::Waiting,
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
