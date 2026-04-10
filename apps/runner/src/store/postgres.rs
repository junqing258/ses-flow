use sqlx::{postgres::PgPool, Row};

use crate::core::runtime::{WorkflowRunSnapshot, WorkflowRunSummary};
use crate::error::RunnerError;

use super::WorkflowRunStore;

pub struct PostgresRunStore {
    pool: PgPool,
}

impl PostgresRunStore {
    pub async fn new(database_url: &str) -> Result<Self, RunnerError> {
        let pool = PgPool::connect(database_url)
            .await
            .map_err(|e| RunnerError::Store(format!("Failed to connect to PostgreSQL database: {}", e)))?;

        let store = Self { pool };
        store.init_schema().await?;
        Ok(store)
    }

    pub fn get_pool(&self) -> PgPool {
        self.pool.clone()
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
                state JSONB NOT NULL,
                timeline JSONB NOT NULL,
                last_signal JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
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
                trigger JSONB NOT NULL,
                last_input JSONB NOT NULL,
                state JSONB NOT NULL,
                timeline JSONB NOT NULL,
                last_signal JSONB,
                env JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
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
impl WorkflowRunStore for PostgresRunStore {
    fn save_summary(&self, summary: &WorkflowRunSummary) -> Result<(), RunnerError> {
        let status_str = serde_json::to_string(&summary.status)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize status: {}", e)))?;

        let state_value = serde_json::to_value(&summary.state)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize state: {}", e)))?;
        let timeline_value = serde_json::to_value(&summary.timeline)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize timeline: {}", e)))?;
        let last_signal_value = serde_json::to_value(&summary.last_signal)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_signal: {}", e)))?;

        let pool = self.pool.clone();
        let run_id = summary.run_id.clone();
        let workflow_key = summary.workflow_key.clone();
        let workflow_version = summary.workflow_version as i32;
        let current_node_id = summary.current_node_id.clone();

        tokio::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO workflow_runs (
                    run_id, workflow_key, workflow_version, status, current_node_id,
                    state, timeline, last_signal
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (run_id) DO UPDATE SET
                    status = EXCLUDED.status,
                    current_node_id = EXCLUDED.current_node_id,
                    state = EXCLUDED.state,
                    timeline = EXCLUDED.timeline,
                    last_signal = EXCLUDED.last_signal,
                    updated_at = NOW()
                "#,
            )
            .bind(&run_id)
            .bind(&workflow_key)
            .bind(workflow_version)
            .bind(&status_str)
            .bind(&current_node_id)
            .bind(state_value)
            .bind(timeline_value)
            .bind(last_signal_value)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::error!(error = %e, "Failed to save summary to PostgreSQL");
            }
        });

        Ok(())
    }

    fn save_snapshot(&self, snapshot: WorkflowRunSnapshot) -> Result<(), RunnerError> {
        let state_value = serde_json::to_value(&snapshot.state)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize state: {}", e)))?;
        let timeline_value = serde_json::to_value(&snapshot.timeline)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize timeline: {}", e)))?;
        let trigger_value = serde_json::to_value(&snapshot.trigger)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize trigger: {}", e)))?;
        let last_input_value = serde_json::to_value(&snapshot.last_input)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_input: {}", e)))?;
        let last_signal_value = serde_json::to_value(&snapshot.last_signal)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_signal: {}", e)))?;
        let env_value = serde_json::to_value(&snapshot.env)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize env: {}", e)))?;

        let pool = self.pool.clone();
        let run_id = snapshot.run_id.clone();
        let workflow_key = snapshot.workflow_key.clone();
        let workflow_version = snapshot.workflow_version as i32;
        let current_node_id = snapshot.current_node_id.clone();

        tokio::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO workflow_snapshots (
                    run_id, workflow_key, workflow_version, current_node_id,
                    trigger, last_input, state, timeline, last_signal, env
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (run_id) DO UPDATE SET
                    workflow_key = EXCLUDED.workflow_key,
                    workflow_version = EXCLUDED.workflow_version,
                    current_node_id = EXCLUDED.current_node_id,
                    trigger = EXCLUDED.trigger,
                    last_input = EXCLUDED.last_input,
                    state = EXCLUDED.state,
                    timeline = EXCLUDED.timeline,
                    last_signal = EXCLUDED.last_signal,
                    env = EXCLUDED.env,
                    updated_at = NOW()
                "#,
            )
            .bind(&run_id)
            .bind(&workflow_key)
            .bind(workflow_version)
            .bind(&current_node_id)
            .bind(trigger_value)
            .bind(last_input_value)
            .bind(state_value)
            .bind(timeline_value)
            .bind(last_signal_value)
            .bind(env_value)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::error!(error = %e, "Failed to save snapshot to PostgreSQL");
            }
        });

        Ok(())
    }

    fn load_snapshot(&self, run_id: &str) -> Result<Option<WorkflowRunSnapshot>, RunnerError> {
        let pool = self.pool.clone();
        let run_id = run_id.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let result: Result<Option<sqlx::postgres::PgRow>, RunnerError> = sqlx::query(
                    r#"
                    SELECT run_id, workflow_key, workflow_version, current_node_id,
                           trigger, last_input, state, timeline, last_signal, env
                    FROM workflow_snapshots
                    WHERE run_id = $1
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
                        let workflow_version: i32 = row.try_get("workflow_version")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
                        let current_node_id: String = row.try_get("current_node_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
                        let trigger: serde_json::Value = row.try_get("trigger")
                            .map_err(|e| RunnerError::Store(format!("Failed to get trigger: {}", e)))?;
                        let last_input: serde_json::Value = row.try_get("last_input")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_input: {}", e)))?;
                        let state: serde_json::Value = row.try_get("state")
                            .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
                        let timeline: serde_json::Value = row.try_get("timeline")
                            .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
                        let last_signal: Option<serde_json::Value> = row.try_get("last_signal")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;
                        let env: serde_json::Value = row.try_get("env")
                            .map_err(|e| RunnerError::Store(format!("Failed to get env: {}", e)))?;

                        let trigger_typed: serde_json::Value = serde_json::from_value(trigger)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize trigger: {}", e)))?;
                        let last_input_typed: serde_json::Value = serde_json::from_value(last_input)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_input: {}", e)))?;
                        let state_typed: serde_json::Value = serde_json::from_value(state)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
                        let timeline_vec: Vec<crate::core::runtime::NodeExecutionRecord> = serde_json::from_value(timeline)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
                        let last_signal_typed: Option<crate::core::runtime::NextSignal> = match last_signal {
                            Some(s) => Some(serde_json::from_value(s)
                                .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_signal: {}", e)))?),
                            None => None,
                        };
                        let env_typed: crate::core::runtime::RunEnvironment = serde_json::from_value(env)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize env: {}", e)))?;

                        Ok(Some(WorkflowRunSnapshot {
                            run_id,
                            workflow_key,
                            workflow_version: workflow_version as u32,
                            current_node_id,
                            trigger: trigger_typed,
                            last_input: last_input_typed,
                            state: state_typed,
                            timeline: timeline_vec,
                            last_signal: last_signal_typed,
                            env: env_typed,
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
                let result: Result<Option<sqlx::postgres::PgRow>, RunnerError> = sqlx::query(
                    r#"
                    SELECT run_id, workflow_key, workflow_version, status, current_node_id,
                           state, timeline, last_signal
                    FROM workflow_runs
                    WHERE run_id = $1
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
                        let workflow_version: i32 = row.try_get("workflow_version")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
                        let status_str: String = row.try_get("status")
                            .map_err(|e| RunnerError::Store(format!("Failed to get status: {}", e)))?;
                        let current_node_id: Option<String> = row.try_get("current_node_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
                        let state: serde_json::Value = row.try_get("state")
                            .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
                        let timeline: serde_json::Value = row.try_get("timeline")
                            .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
                        let last_signal: Option<serde_json::Value> = row.try_get("last_signal")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;

                        let status: crate::core::runtime::WorkflowRunStatus = serde_json::from_str(&status_str)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize status: {}", e)))?;

                        let state_typed: serde_json::Value = serde_json::from_value(state)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
                        let timeline_vec: Vec<crate::core::runtime::NodeExecutionRecord> = serde_json::from_value(timeline)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
                        let last_signal_typed: Option<crate::core::runtime::NextSignal> = match last_signal {
                            Some(s) => Some(serde_json::from_value(s)
                                .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_signal: {}", e)))?),
                            None => None,
                        };

                        Ok(Some(WorkflowRunSummary {
                            run_id,
                            workflow_key,
                            workflow_version: workflow_version as u32,
                            status,
                            current_node_id,
                            state: state_typed,
                            timeline: timeline_vec,
                            last_signal: last_signal_typed,
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

        let state_value = serde_json::to_value(&summary.state)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize state: {}", e)))?;
        let timeline_value = serde_json::to_value(&summary.timeline)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize timeline: {}", e)))?;
        let last_signal_value = serde_json::to_value(&summary.last_signal)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize last_signal: {}", e)))?;

        let is_completed = matches!(summary.status, crate::core::runtime::WorkflowRunStatus::Completed | crate::core::runtime::WorkflowRunStatus::Failed);

        let pool = self.pool.clone();
        let run_id = summary.run_id.clone();
        let workflow_key = summary.workflow_key.clone();
        let workflow_version = summary.workflow_version as i32;
        let current_node_id = summary.current_node_id.clone();

        tokio::spawn(async move {
            let result = sqlx::query(
                r#"
                INSERT INTO workflow_runs (
                    run_id, workflow_key, workflow_version, status, current_node_id,
                    state, timeline, last_signal
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (run_id) DO UPDATE SET
                    status = EXCLUDED.status,
                    current_node_id = EXCLUDED.current_node_id,
                    state = EXCLUDED.state,
                    timeline = EXCLUDED.timeline,
                    last_signal = EXCLUDED.last_signal,
                    updated_at = NOW()
                "#,
            )
            .bind(&run_id)
            .bind(&workflow_key)
            .bind(workflow_version)
            .bind(&status_str)
            .bind(&current_node_id)
            .bind(state_value)
            .bind(timeline_value)
            .bind(last_signal_value)
            .execute(&pool)
            .await;

            if let Err(e) = result {
                tracing::error!(error = %e, "Failed to update summary in PostgreSQL");
            }

            // Remove snapshot if workflow is completed or failed
            if is_completed {
                let delete_result = sqlx::query(
                    r#"
                    DELETE FROM workflow_snapshots WHERE run_id = $1
                    "#,
                )
                .bind(&run_id)
                .execute(&pool)
                .await;

                if let Err(e) = delete_result {
                    tracing::error!(error = %e, "Failed to delete snapshot from PostgreSQL");
                }
            }
        });

        Ok(())
    }
}
