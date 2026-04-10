use sqlx::{sqlite::SqlitePool, Row};
use std::path::Path;

use crate::core::runtime::{WorkflowRunSnapshot, WorkflowRunSummary};
use crate::error::RunnerError;

use super::WorkflowRunStore;

pub struct SqliteRunStore {
    pool: SqlitePool,
}

impl SqliteRunStore {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, RunnerError> {
        let connection_string = format!("sqlite://{}?mode=rwc", db_path.as_ref().display());
        let pool = SqlitePool::connect(&connection_string)
            .await
            .map_err(|e| RunnerError::Store(format!("Failed to connect to SQLite database: {}", e)))?;

        let store = Self { pool };
        store.init_schema().await?;
        Ok(store)
    }

    async fn init_schema(&self) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_runs (
                run_id TEXT PRIMARY KEY,
                workflow_key TEXT NOT NULL,
                workflow_version INTEGER NOT NULL,
                status TEXT NOT NULL,
                current_node_id TEXT,
                state TEXT NOT NULL,
                timeline TEXT NOT NULL,
                last_signal TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create workflow_runs table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_snapshots (
                run_id TEXT PRIMARY KEY,
                workflow_key TEXT NOT NULL,
                workflow_version INTEGER NOT NULL,
                current_node_id TEXT NOT NULL,
                trigger TEXT NOT NULL,
                last_input TEXT NOT NULL,
                state TEXT NOT NULL,
                timeline TEXT NOT NULL,
                last_signal TEXT,
                env TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create workflow_snapshots table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_status ON workflow_runs(status)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_workflow_key ON workflow_runs(workflow_key)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl WorkflowRunStore for SqliteRunStore {
    fn save_summary(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let status_str = serde_json::to_string(&summary.status)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize status: {}", e)))?;

        let state_json = serde_json::to_string(&summary.state)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize state: {}", e)))?;

        let timeline_json = serde_json::to_string(&summary.timeline)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize timeline: {}", e)))?;

        let last_signal_json = summary.last_signal.as_ref()
            .map(|s| serde_json::to_string(s))
            .transpose()
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_signal: {}", e)))?;

        let now = chrono::Utc::now().to_rfc3339();

        let pool = self.pool.clone();
        let run_id = summary.run_id.clone();
        let workflow_key = summary.workflow_key.clone();
        let workflow_version = summary.workflow_version;
        let current_node_id = summary.current_node_id.clone();
        let last_signal = last_signal_json;

        tokio::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO workflow_runs (
                    run_id, workflow_key, workflow_version, status, current_node_id,
                    state, timeline, last_signal, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(run_id) DO UPDATE SET
                    status = excluded.status,
                    current_node_id = excluded.current_node_id,
                    state = excluded.state,
                    timeline = excluded.timeline,
                    last_signal = excluded.last_signal,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&run_id)
            .bind(&workflow_key)
            .bind(workflow_version)
            .bind(&status_str)
            .bind(&current_node_id)
            .bind(&state_json)
            .bind(&timeline_json)
            .bind(&last_signal)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::error!(error = %e, "Failed to save summary to SQLite");
            }
        });

        Ok(())
    }

    fn save_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError> {
        let state_json = serde_json::to_string(&snapshot.state)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize state: {}", e)))?;

        let timeline_json = serde_json::to_string(&snapshot.timeline)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize timeline: {}", e)))?;

        let trigger_json = serde_json::to_string(&snapshot.trigger)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize trigger: {}", e)))?;

        let last_input_json = serde_json::to_string(&snapshot.last_input)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_input: {}", e)))?;

        let last_signal_json = snapshot.last_signal.as_ref()
            .map(|s| serde_json::to_string(s))
            .transpose()
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_signal: {}", e)))?;

        let env_json = serde_json::to_string(&snapshot.env)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize env: {}", e)))?;

        let now = chrono::Utc::now().to_rfc3339();

        let pool = self.pool.clone();
        let run_id = snapshot.run_id.clone();
        let workflow_key = snapshot.workflow_key.clone();
        let workflow_version = snapshot.workflow_version;
        let current_node_id = snapshot.current_node_id.clone();
        let last_signal = last_signal_json;

        tokio::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO workflow_snapshots (
                    run_id, workflow_key, workflow_version, current_node_id,
                    trigger, last_input, state, timeline, last_signal, env,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(run_id) DO UPDATE SET
                    workflow_key = excluded.workflow_key,
                    workflow_version = excluded.workflow_version,
                    current_node_id = excluded.current_node_id,
                    trigger = excluded.trigger,
                    last_input = excluded.last_input,
                    state = excluded.state,
                    timeline = excluded.timeline,
                    last_signal = excluded.last_signal,
                    env = excluded.env,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&run_id)
            .bind(&workflow_key)
            .bind(workflow_version)
            .bind(&current_node_id)
            .bind(&trigger_json)
            .bind(&last_input_json)
            .bind(&state_json)
            .bind(&timeline_json)
            .bind(&last_signal)
            .bind(&env_json)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::error!(error = %e, "Failed to save snapshot to SQLite");
            }
        });

        Ok(())
    }

    fn load_snapshot(&self, run_id: &str) -> Result<Option<WorkflowRunSnapshot>, RunnerError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let result: Result<Option<sqlx::sqlite::SqliteRow>, RunnerError> = sqlx::query(
                    r#"
                    SELECT run_id, workflow_key, workflow_version, current_node_id,
                           trigger, last_input, state, timeline, last_signal, env
                    FROM workflow_snapshots
                    WHERE run_id = ?
                    "#,
                )
                .bind(&run_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to load snapshot: {}", e)));

                let row = result?;

                let snapshot = match row {
                    Some(row) => {
                        let run_id: String = row.try_get("run_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?;
                        let workflow_key: String = row.try_get("workflow_key")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?;
                        let workflow_version: u32 = row.try_get("workflow_version")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
                        let current_node_id: String = row.try_get("current_node_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
                        let trigger: String = row.try_get("trigger")
                            .map_err(|e| RunnerError::Store(format!("Failed to get trigger: {}", e)))?;
                        let last_input: String = row.try_get("last_input")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_input: {}", e)))?;
                        let state: String = row.try_get("state")
                            .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
                        let timeline: String = row.try_get("timeline")
                            .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
                        let last_signal: Option<String> = row.try_get("last_signal")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;
                        let env: String = row.try_get("env")
                            .map_err(|e| RunnerError::Store(format!("Failed to get env: {}", e)))?;

                        let trigger_value = serde_json::from_str(&trigger)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize trigger: {}", e)))?;
                        let last_input_value = serde_json::from_str(&last_input)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_input: {}", e)))?;
                        let state_value = serde_json::from_str(&state)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
                        let timeline_value = serde_json::from_str(&timeline)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
                        let last_signal_value = match last_signal {
                            Some(s) => Some(serde_json::from_str(&s)
                                .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_signal: {}", e)))?),
                            None => None,
                        };
                        let env_value = serde_json::from_str(&env)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize env: {}", e)))?;

                        Ok(Some(WorkflowRunSnapshot {
                            run_id,
                            workflow_key,
                            workflow_version,
                            current_node_id,
                            trigger: trigger_value,
                            last_input: last_input_value,
                            state: state_value,
                            timeline: timeline_value,
                            last_signal: last_signal_value,
                            env: env_value,
                        }))
                    }
                    None => Ok(None),
                };

                snapshot
            })
        })
    }

    fn load_summary(&self, run_id: &str) -> Result<Option<WorkflowRunSummary>, RunnerError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let result: Result<Option<sqlx::sqlite::SqliteRow>, RunnerError> = sqlx::query(
                    r#"
                    SELECT run_id, workflow_key, workflow_version, status, current_node_id,
                           state, timeline, last_signal
                    FROM workflow_runs
                    WHERE run_id = ?
                    "#,
                )
                .bind(&run_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to load summary: {}", e)));

                let row = result?;

                let summary = match row {
                    Some(row) => {
                        let run_id: String = row.try_get("run_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?;
                        let workflow_key: String = row.try_get("workflow_key")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?;
                        let workflow_version: u32 = row.try_get("workflow_version")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
                        let status_str: String = row.try_get("status")
                            .map_err(|e| RunnerError::Store(format!("Failed to get status: {}", e)))?;
                        let current_node_id: Option<String> = row.try_get("current_node_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
                        let state: String = row.try_get("state")
                            .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
                        let timeline: String = row.try_get("timeline")
                            .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
                        let last_signal: Option<String> = row.try_get("last_signal")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;

                        let status_value = serde_json::from_str(&status_str)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize status: {}", e)))?;
                        let state_value = serde_json::from_str(&state)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
                        let timeline_value = serde_json::from_str(&timeline)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
                        let last_signal_value = match last_signal {
                            Some(s) => Some(serde_json::from_str(&s)
                                .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_signal: {}", e)))?),
                            None => None,
                        };

                        Ok(Some(WorkflowRunSummary {
                            run_id,
                            workflow_key,
                            workflow_version,
                            status: status_value,
                            current_node_id,
                            state: state_value,
                            timeline: timeline_value,
                            last_signal: last_signal_value,
                            resume_state: None,
                        }))
                    }
                    None => Ok(None),
                };

                summary
            })
        })
    }

    fn mark_completed(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let status_str = serde_json::to_string(&summary.status)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize status: {}", e)))?;

        let state_json = serde_json::to_string(&summary.state)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize state: {}", e)))?;

        let timeline_json = serde_json::to_string(&summary.timeline)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize timeline: {}", e)))?;

        let last_signal_json = summary.last_signal.as_ref()
            .map(|s| serde_json::to_string(s))
            .transpose()
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_signal: {}", e)))?;

        let now = chrono::Utc::now().to_rfc3339();

        let pool = self.pool.clone();
        let run_id = summary.run_id.clone();
        let workflow_key = summary.workflow_key.clone();
        let workflow_version = summary.workflow_version;
        let current_node_id = summary.current_node_id.clone();
        let last_signal = last_signal_json;
        let is_completed = matches!(summary.status, crate::core::runtime::WorkflowRunStatus::Completed | crate::core::runtime::WorkflowRunStatus::Failed);

        tokio::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO workflow_runs (
                    run_id, workflow_key, workflow_version, status, current_node_id,
                    state, timeline, last_signal, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(run_id) DO UPDATE SET
                    status = excluded.status,
                    current_node_id = excluded.current_node_id,
                    state = excluded.state,
                    timeline = excluded.timeline,
                    last_signal = excluded.last_signal,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&run_id)
            .bind(&workflow_key)
            .bind(workflow_version)
            .bind(&status_str)
            .bind(&current_node_id)
            .bind(&state_json)
            .bind(&timeline_json)
            .bind(&last_signal)
            .bind(&now)
            .bind(&now)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::error!(error = %e, "Failed to update summary in SQLite");
            }

            // Remove snapshot if workflow is completed or failed
            if is_completed {
                let delete_result = sqlx::query(
                    r#"
                    DELETE FROM workflow_snapshots WHERE run_id = ?
                    "#,
                )
                .bind(&run_id)
                .execute(&pool)
                .await;

                if let Err(e) = delete_result {
                    tracing::error!(error = %e, "Failed to delete snapshot from SQLite");
                }
            }
        });

        Ok(())
    }
}
