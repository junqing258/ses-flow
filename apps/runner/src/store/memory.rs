use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::core::runtime::{WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary};
use crate::error::RunnerError;

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRunRecord {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
    pub status: WorkflowRunStatus,
    #[serde(rename = "currentNodeId", skip_serializing_if = "Option::is_none")]
    pub current_node_id: Option<String>,
    #[serde(rename = "orderNo", skip_serializing_if = "Option::is_none")]
    pub order_no: Option<String>,
    #[serde(rename = "waveNo", skip_serializing_if = "Option::is_none")]
    pub wave_no: Option<String>,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowRunLookup {
    pub run_id: String,
    pub order_no: Option<String>,
    pub wave_no: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowRunSearchQuery {
    pub run_id: Option<String>,
    pub order_no: Option<String>,
    pub wave_no: Option<String>,
    pub request_id: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRunSearchResult {
    pub items: Vec<WorkflowRunRecord>,
    pub total: usize,
}

pub trait WorkflowRunStore: Send + Sync {
    fn save_summary(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError>;
    fn save_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError>;
    fn load_snapshot(&self, run_id: &str) -> Result<Option<WorkflowRunSnapshot>, RunnerError>;
    fn load_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, RunnerError>;
    fn list_runs(&self, workflow_key: &str, workflow_version: u32) -> Result<Vec<WorkflowRunRecord>, RunnerError>;
    fn register_run_lookup(&self, lookup: WorkflowRunLookup) -> Result<(), RunnerError>;
    fn search_runs(&self, query: &WorkflowRunSearchQuery) -> Result<WorkflowRunSearchResult, RunnerError>;
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
    timestamps: HashMap<String, RunTimestamps>,
    lookups: HashMap<String, WorkflowRunLookup>,
}

#[derive(Clone, Copy)]
struct RunTimestamps {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl InMemoryRunStore {
    pub fn new() -> Self {
        Self::default()
    }
}

fn includes_text(haystack: &str, needle: &str) -> bool {
    haystack.to_ascii_lowercase().contains(&needle.to_ascii_lowercase())
}

fn matches_optional_field(field: &Option<String>, expected: &Option<String>) -> bool {
    match expected {
        Some(expected) => field.as_deref().is_some_and(|value| includes_text(value, expected)),
        None => true,
    }
}

impl WorkflowRunStore for InMemoryRunStore {
    fn save_summary(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        let now = Utc::now();
        let timestamps = state.timestamps.entry(summary.run_id.clone()).or_insert(RunTimestamps {
            created_at: now,
            updated_at: now,
        });
        timestamps.updated_at = now;
        state.summaries.insert(summary.run_id.clone(), summary.clone());
        Ok(())
    }

    fn save_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        state.waiting_snapshots.insert(snapshot.run_id.clone(), snapshot);
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

    fn list_runs(&self, workflow_key: &str, workflow_version: u32) -> Result<Vec<WorkflowRunRecord>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        let mut runs = state
            .summaries
            .values()
            .filter(|summary| summary.workflow_key == workflow_key && summary.workflow_version == workflow_version)
            .filter_map(|summary| {
                let timestamps = state.timestamps.get(&summary.run_id)?;
                let lookup = state.lookups.get(&summary.run_id);
                Some(WorkflowRunRecord {
                    run_id: summary.run_id.clone(),
                    workflow_key: summary.workflow_key.clone(),
                    workflow_version: summary.workflow_version,
                    status: summary.status.clone(),
                    current_node_id: summary.current_node_id.clone(),
                    order_no: lookup.and_then(|item| item.order_no.clone()),
                    wave_no: lookup.and_then(|item| item.wave_no.clone()),
                    request_id: lookup.and_then(|item| item.request_id.clone()),
                    created_at: timestamps.created_at,
                    updated_at: timestamps.updated_at,
                })
            })
            .collect::<Vec<_>>();

        runs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(runs)
    }

    fn register_run_lookup(&self, lookup: WorkflowRunLookup) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        state.lookups.insert(lookup.run_id.clone(), lookup);
        Ok(())
    }

    fn search_runs(&self, query: &WorkflowRunSearchQuery) -> Result<WorkflowRunSearchResult, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;

        let mut runs = state
            .summaries
            .values()
            .filter(|summary| {
                query
                    .run_id
                    .as_deref()
                    .map_or(true, |run_id| includes_text(&summary.run_id, run_id))
            })
            .filter_map(|summary| {
                let timestamps = state.timestamps.get(&summary.run_id)?;
                let lookup = state.lookups.get(&summary.run_id).cloned().unwrap_or_default();

                if !matches_optional_field(&lookup.order_no, &query.order_no)
                    || !matches_optional_field(&lookup.wave_no, &query.wave_no)
                    || !matches_optional_field(&lookup.request_id, &query.request_id)
                {
                    return None;
                }

                Some(WorkflowRunRecord {
                    run_id: summary.run_id.clone(),
                    workflow_key: summary.workflow_key.clone(),
                    workflow_version: summary.workflow_version,
                    status: summary.status.clone(),
                    current_node_id: summary.current_node_id.clone(),
                    order_no: lookup.order_no,
                    wave_no: lookup.wave_no,
                    request_id: lookup.request_id,
                    created_at: timestamps.created_at,
                    updated_at: timestamps.updated_at,
                })
            })
            .collect::<Vec<_>>();

        runs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        let total = runs.len();
        let page = query.page.max(1) as usize;
        let page_size = query.page_size.max(1) as usize;
        let start = (page - 1).saturating_mul(page_size);
        let items = runs.into_iter().skip(start).take(page_size).collect();

        Ok(WorkflowRunSearchResult { items, total })
    }

    fn mark_completed(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("failed to lock in-memory run store".to_string()))?;
        let now = Utc::now();
        let timestamps = state.timestamps.entry(summary.run_id.clone()).or_insert(RunTimestamps {
            created_at: now,
            updated_at: now,
        });
        timestamps.updated_at = now;
        state.summaries.insert(summary.run_id.clone(), summary.clone());
        if matches!(
            summary.status,
            WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Terminated
        ) {
            state.waiting_snapshots.remove(&summary.run_id);
        }
        Ok(())
    }
}
