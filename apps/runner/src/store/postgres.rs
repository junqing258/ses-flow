use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use sqlx::{
    QueryBuilder, Row,
    postgres::{PgPool, PgRow},
};

use crate::core::runtime::{WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary};
use crate::error::RunnerError;

use super::{WorkflowRunLookup, WorkflowRunRecord, WorkflowRunSearchQuery, WorkflowRunSearchResult, WorkflowRunStore};

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

        sqlx::query(
            r#"
            ALTER TABLE workflow_runs
                ADD COLUMN IF NOT EXISTS order_no TEXT,
                ADD COLUMN IF NOT EXISTS wave_no TEXT,
                ADD COLUMN IF NOT EXISTS request_id TEXT,
                ADD COLUMN IF NOT EXISTS unique_key TEXT
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to alter workflow_runs table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_order_no ON workflow_runs(order_no)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_wave_no ON workflow_runs(wave_no)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_request_id ON workflow_runs(request_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_unique_key ON workflow_runs(unique_key)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_timeline_gin ON workflow_runs USING GIN (timeline)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS chute_status (
                chute_id        TEXT        PRIMARY KEY,
                platform_id     TEXT        NOT NULL,
                wave_no         TEXT,
                current_count   INTEGER     NOT NULL DEFAULT 0,
                capacity        INTEGER     NOT NULL DEFAULT 100,
                is_full         BOOLEAN     NOT NULL DEFAULT FALSE,
                pack_run_id     TEXT,
                updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create chute_status table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_chute_status_platform_id ON chute_status (platform_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wave (
                wave_no         TEXT        PRIMARY KEY,
                platform_id     TEXT        NOT NULL,
                chute_id        TEXT        NOT NULL,
                rule_version    TEXT,
                status          TEXT        NOT NULL DEFAULT 'active',
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create wave table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wave_order (
                wave_no         TEXT        NOT NULL,
                run_id          TEXT        NOT NULL,
                order_id        TEXT        NOT NULL,
                status          TEXT        NOT NULL DEFAULT 'sorting',
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (wave_no, run_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create wave_order table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_wave_order_wave_no ON wave_order (wave_no)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_wave_order_run_id ON wave_order (run_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sorting_flow (
                id              BIGSERIAL   PRIMARY KEY,
                task_id         TEXT        NOT NULL,
                run_id          TEXT        NOT NULL,
                platform_id     TEXT        NOT NULL,
                wave_no         TEXT,
                order_id        TEXT,
                chute_id        TEXT        NOT NULL,
                sku             TEXT,
                barcode         TEXT,
                sorting_result  TEXT        NOT NULL DEFAULT 'ok',
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create sorting_flow table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sorting_flow_run_id ON sorting_flow (run_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sorting_flow_wave_no ON sorting_flow (wave_no)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sorting_flow_platform_id ON sorting_flow (platform_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sorting_flow_created_at ON sorting_flow (created_at)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        Ok(())
    }
}

fn deserialize_optional_json_field<T>(
    value: Option<serde_json::Value>,
    field_name: &str,
) -> Result<Option<T>, RunnerError>
where
    T: DeserializeOwned,
{
    match value {
        Some(serde_json::Value::Null) | None => Ok(None),
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|e| RunnerError::Store(format!("Failed to deserialize {field_name}: {e}"))),
    }
}

fn workflow_run_summary_from_row(row: PgRow) -> Result<WorkflowRunSummary, RunnerError> {
    let run_id: String = row
        .try_get("run_id")
        .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?;
    let workflow_key: String = row
        .try_get("workflow_key")
        .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?;
    let workflow_version: i32 = row
        .try_get("workflow_version")
        .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
    let status_str: String = row
        .try_get("status")
        .map_err(|e| RunnerError::Store(format!("Failed to get status: {}", e)))?;
    let current_node_id: Option<String> = row
        .try_get("current_node_id")
        .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
    let state: serde_json::Value = row
        .try_get("state")
        .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
    let timeline: serde_json::Value = row
        .try_get("timeline")
        .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
    let last_signal: Option<serde_json::Value> = row
        .try_get("last_signal")
        .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;

    let status: WorkflowRunStatus = serde_json::from_str(&status_str)
        .map_err(|e| RunnerError::Store(format!("Failed to deserialize status: {}", e)))?;
    let state_typed: serde_json::Value =
        serde_json::from_value(state).map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
    let timeline_vec: Vec<crate::core::runtime::NodeExecutionRecord> = serde_json::from_value(timeline)
        .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
    let last_signal_typed: Option<crate::core::runtime::NextSignal> =
        deserialize_optional_json_field(last_signal, "last_signal")?;

    Ok(WorkflowRunSummary {
        run_id,
        workflow_key,
        workflow_version: workflow_version as u32,
        status,
        current_node_id,
        state: state_typed,
        timeline: timeline_vec,
        last_signal: last_signal_typed,
        resume_state: None,
    })
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

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query(
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
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to save summary: {}", e)))?;

                Ok(())
            })
        })
    }

    fn save_started_summary(
        &self,
        summary: &WorkflowRunSummary,
        lookup: WorkflowRunLookup,
    ) -> Result<Option<WorkflowRunSummary>, RunnerError> {
        let status_str = serde_json::to_string(&summary.status)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize status: {}", e)))?;
        let running_status = serde_json::to_string(&WorkflowRunStatus::Running)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize running status: {}", e)))?;
        let waiting_status = serde_json::to_string(&WorkflowRunStatus::Waiting)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize waiting status: {}", e)))?;

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

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let mut tx = pool
                    .begin()
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to start workflow run transaction: {}", e)))?;

                if let Some(unique_key) = lookup.unique_key.as_deref() {
                    let lock_key = format!("{workflow_key}:{workflow_version}:{unique_key}");
                    sqlx::query(
                        r#"
                        SELECT pg_advisory_xact_lock(hashtext($1), hashtext($2))
                        "#,
                    )
                    .bind("workflow_runs_active_order")
                    .bind(&lock_key)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to lock workflow run idempotency key: {}", e)))?;

                    let existing = sqlx::query(
                        r#"
                        SELECT run_id, workflow_key, workflow_version, status, current_node_id,
                               state, timeline, last_signal
                        FROM workflow_runs
                        WHERE workflow_key = $1
                          AND workflow_version = $2
                          AND unique_key = $3
                          AND status IN ($4, $5)
                        ORDER BY created_at ASC
                        LIMIT 1
                        "#,
                    )
                    .bind(&workflow_key)
                    .bind(workflow_version)
                    .bind(unique_key)
                    .bind(&running_status)
                    .bind(&waiting_status)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to find active workflow run: {}", e)))?;

                    if let Some(row) = existing {
                        let summary = workflow_run_summary_from_row(row)?;
                        tx.commit().await.map_err(|e| {
                            RunnerError::Store(format!("Failed to commit workflow run transaction: {}", e))
                        })?;
                        return Ok(Some(summary));
                    }
                }

                sqlx::query(
                    r#"
                    INSERT INTO workflow_runs (
                        run_id, workflow_key, workflow_version, status, current_node_id,
                        state, timeline, last_signal, order_no, wave_no, request_id, unique_key
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ON CONFLICT (run_id) DO UPDATE SET
                        status = EXCLUDED.status,
                        current_node_id = EXCLUDED.current_node_id,
                        state = EXCLUDED.state,
                        timeline = EXCLUDED.timeline,
                        last_signal = EXCLUDED.last_signal,
                        order_no = EXCLUDED.order_no,
                        wave_no = EXCLUDED.wave_no,
                        request_id = EXCLUDED.request_id,
                        unique_key = EXCLUDED.unique_key,
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
                .bind(&lookup.order_no)
                .bind(&lookup.wave_no)
                .bind(&lookup.request_id)
                .bind(&lookup.unique_key)
                .execute(&mut *tx)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to save started summary: {}", e)))?;

                tx.commit()
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to commit workflow run transaction: {}", e)))?;

                Ok(None)
            })
        })
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

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query(
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
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to save snapshot: {}", e)))?;

                Ok(())
            })
        })
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
                        let run_id: String = row
                            .try_get("run_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?;
                        let workflow_key: String = row
                            .try_get("workflow_key")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?;
                        let workflow_version: i32 = row
                            .try_get("workflow_version")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
                        let current_node_id: String = row
                            .try_get("current_node_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
                        let trigger: serde_json::Value = row
                            .try_get("trigger")
                            .map_err(|e| RunnerError::Store(format!("Failed to get trigger: {}", e)))?;
                        let last_input: serde_json::Value = row
                            .try_get("last_input")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_input: {}", e)))?;
                        let state: serde_json::Value = row
                            .try_get("state")
                            .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
                        let timeline: serde_json::Value = row
                            .try_get("timeline")
                            .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
                        let last_signal: Option<serde_json::Value> = row
                            .try_get("last_signal")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;
                        let env: serde_json::Value = row
                            .try_get("env")
                            .map_err(|e| RunnerError::Store(format!("Failed to get env: {}", e)))?;

                        let trigger_typed: serde_json::Value = serde_json::from_value(trigger)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize trigger: {}", e)))?;
                        let last_input_typed: serde_json::Value = serde_json::from_value(last_input)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize last_input: {}", e)))?;
                        let state_typed: serde_json::Value = serde_json::from_value(state)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
                        let timeline_vec: Vec<crate::core::runtime::NodeExecutionRecord> =
                            serde_json::from_value(timeline)
                                .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
                        let last_signal_typed: Option<crate::core::runtime::NextSignal> =
                            deserialize_optional_json_field(last_signal, "last_signal")?;
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
                        let run_id: String = row
                            .try_get("run_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?;
                        let workflow_key: String = row
                            .try_get("workflow_key")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?;
                        let workflow_version: i32 = row
                            .try_get("workflow_version")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?;
                        let status_str: String = row
                            .try_get("status")
                            .map_err(|e| RunnerError::Store(format!("Failed to get status: {}", e)))?;
                        let current_node_id: Option<String> = row
                            .try_get("current_node_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?;
                        let state: serde_json::Value = row
                            .try_get("state")
                            .map_err(|e| RunnerError::Store(format!("Failed to get state: {}", e)))?;
                        let timeline: serde_json::Value = row
                            .try_get("timeline")
                            .map_err(|e| RunnerError::Store(format!("Failed to get timeline: {}", e)))?;
                        let last_signal: Option<serde_json::Value> = row
                            .try_get("last_signal")
                            .map_err(|e| RunnerError::Store(format!("Failed to get last_signal: {}", e)))?;

                        let status: crate::core::runtime::WorkflowRunStatus = serde_json::from_str(&status_str)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize status: {}", e)))?;

                        let state_typed: serde_json::Value = serde_json::from_value(state)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize state: {}", e)))?;
                        let timeline_vec: Vec<crate::core::runtime::NodeExecutionRecord> =
                            serde_json::from_value(timeline)
                                .map_err(|e| RunnerError::Store(format!("Failed to deserialize timeline: {}", e)))?;
                        let last_signal_typed: Option<crate::core::runtime::NextSignal> =
                            deserialize_optional_json_field(last_signal, "last_signal")?;

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

    fn list_runs(&self, workflow_key: &str, workflow_version: u32) -> Result<Vec<WorkflowRunRecord>, RunnerError> {
        let pool = self.pool.clone();
        let workflow_key = workflow_key.to_string();
        let workflow_version = workflow_version as i32;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let rows = sqlx::query(
                    r#"
                    SELECT run_id, workflow_key, workflow_version, status, current_node_id,
                           order_no, wave_no, request_id, unique_key, created_at, updated_at
                    FROM workflow_runs
                    WHERE workflow_key = $1 AND workflow_version = $2
                    ORDER BY updated_at DESC, created_at DESC
                    "#,
                )
                .bind(&workflow_key)
                .bind(workflow_version)
                .fetch_all(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to list workflow runs: {}", e)))?;

                rows.into_iter()
                    .map(|row| {
                        let status_str: String = row
                            .try_get("status")
                            .map_err(|e| RunnerError::Store(format!("Failed to get status: {}", e)))?;

                        let status: WorkflowRunStatus = serde_json::from_str(&status_str)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize status: {}", e)))?;

                        Ok(WorkflowRunRecord {
                            run_id: row
                                .try_get("run_id")
                                .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?,
                            workflow_key: row
                                .try_get("workflow_key")
                                .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?,
                            workflow_version: row
                                .try_get::<i32, _>("workflow_version")
                                .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?
                                as u32,
                            status,
                            current_node_id: row
                                .try_get("current_node_id")
                                .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?,
                            unique_key: row
                                .try_get("unique_key")
                                .map_err(|e| RunnerError::Store(format!("Failed to get unique_key: {}", e)))?,
                            order_no: row
                                .try_get("order_no")
                                .map_err(|e| RunnerError::Store(format!("Failed to get order_no: {}", e)))?,
                            wave_no: row
                                .try_get("wave_no")
                                .map_err(|e| RunnerError::Store(format!("Failed to get wave_no: {}", e)))?,
                            request_id: row
                                .try_get("request_id")
                                .map_err(|e| RunnerError::Store(format!("Failed to get request_id: {}", e)))?,
                            created_at: row
                                .try_get::<DateTime<Utc>, _>("created_at")
                                .map_err(|e| RunnerError::Store(format!("Failed to get created_at: {}", e)))?,
                            updated_at: row
                                .try_get::<DateTime<Utc>, _>("updated_at")
                                .map_err(|e| RunnerError::Store(format!("Failed to get updated_at: {}", e)))?,
                        })
                    })
                    .collect()
            })
        })
    }

    fn register_run_lookup(&self, lookup: WorkflowRunLookup) -> Result<(), RunnerError> {
        let pool = self.pool.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query(
                    r#"
                    UPDATE workflow_runs
                    SET order_no = $2,
                        wave_no = $3,
                        request_id = $4,
                        unique_key = $5,
                        updated_at = NOW()
                    WHERE run_id = $1
                    "#,
                )
                .bind(&lookup.run_id)
                .bind(&lookup.order_no)
                .bind(&lookup.wave_no)
                .bind(&lookup.request_id)
                .bind(&lookup.unique_key)
                .execute(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to register run lookup: {}", e)))?;

                Ok(())
            })
        })
    }

    fn search_runs(&self, query: &WorkflowRunSearchQuery) -> Result<WorkflowRunSearchResult, RunnerError> {
        let pool = self.pool.clone();
        let query = query.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let mut count_builder = QueryBuilder::new("SELECT COUNT(*) AS total FROM workflow_runs WHERE 1=1");
                let mut select_builder = QueryBuilder::new(
                    "SELECT run_id, workflow_key, workflow_version, status, current_node_id, order_no, wave_no, request_id, unique_key, created_at, updated_at FROM workflow_runs WHERE 1=1",
                );

                if let Some(run_id) = query.run_id.as_deref() {
                    let pattern = format!("%{run_id}%");
                    count_builder.push(" AND run_id ILIKE ").push_bind(pattern.clone());
                    select_builder.push(" AND run_id ILIKE ").push_bind(pattern);
                }

                if let Some(order_no) = query.order_no.as_deref() {
                    let pattern = format!("%{order_no}%");
                    count_builder.push(" AND order_no ILIKE ").push_bind(pattern.clone());
                    select_builder.push(" AND order_no ILIKE ").push_bind(pattern);
                }

                if let Some(wave_no) = query.wave_no.as_deref() {
                    let pattern = format!("%{wave_no}%");
                    count_builder.push(" AND wave_no ILIKE ").push_bind(pattern.clone());
                    select_builder.push(" AND wave_no ILIKE ").push_bind(pattern);
                }

                if let Some(request_id) = query.request_id.as_deref() {
                    let pattern = format!("%{request_id}%");
                    count_builder.push(" AND request_id ILIKE ").push_bind(pattern.clone());
                    select_builder.push(" AND request_id ILIKE ").push_bind(pattern);
                }

                let total_row = count_builder
                    .build()
                    .fetch_one(&pool)
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to count workflow runs: {}", e)))?;
                let total: i64 = total_row
                    .try_get("total")
                    .map_err(|e| RunnerError::Store(format!("Failed to get total: {}", e)))?;

                let page = query.page.max(1) as i64;
                let page_size = query.page_size.max(1) as i64;
                let offset = (page - 1) * page_size;
                select_builder
                    .push(" ORDER BY updated_at DESC, created_at DESC LIMIT ")
                    .push_bind(page_size)
                    .push(" OFFSET ")
                    .push_bind(offset);

                let rows = select_builder
                    .build()
                    .fetch_all(&pool)
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to search workflow runs: {}", e)))?;

                let items = rows
                    .into_iter()
                    .map(|row| {
                        let status_str: String = row
                            .try_get("status")
                            .map_err(|e| RunnerError::Store(format!("Failed to get status: {}", e)))?;
                        let status: WorkflowRunStatus = serde_json::from_str(&status_str)
                            .map_err(|e| RunnerError::Store(format!("Failed to deserialize status: {}", e)))?;

                        Ok(WorkflowRunRecord {
                            run_id: row
                                .try_get("run_id")
                                .map_err(|e| RunnerError::Store(format!("Failed to get run_id: {}", e)))?,
                            workflow_key: row
                                .try_get("workflow_key")
                                .map_err(|e| RunnerError::Store(format!("Failed to get workflow_key: {}", e)))?,
                            workflow_version: row
                                .try_get::<i32, _>("workflow_version")
                                .map_err(|e| RunnerError::Store(format!("Failed to get workflow_version: {}", e)))?
                                as u32,
                            status,
                            current_node_id: row
                                .try_get("current_node_id")
                                .map_err(|e| RunnerError::Store(format!("Failed to get current_node_id: {}", e)))?,
                            unique_key: row
                                .try_get("unique_key")
                                .map_err(|e| RunnerError::Store(format!("Failed to get unique_key: {}", e)))?,
                            order_no: row
                                .try_get("order_no")
                                .map_err(|e| RunnerError::Store(format!("Failed to get order_no: {}", e)))?,
                            wave_no: row
                                .try_get("wave_no")
                                .map_err(|e| RunnerError::Store(format!("Failed to get wave_no: {}", e)))?,
                            request_id: row
                                .try_get("request_id")
                                .map_err(|e| RunnerError::Store(format!("Failed to get request_id: {}", e)))?,
                            created_at: row
                                .try_get::<DateTime<Utc>, _>("created_at")
                                .map_err(|e| RunnerError::Store(format!("Failed to get created_at: {}", e)))?,
                            updated_at: row
                                .try_get::<DateTime<Utc>, _>("updated_at")
                                .map_err(|e| RunnerError::Store(format!("Failed to get updated_at: {}", e)))?,
                        })
                    })
                    .collect::<Result<Vec<_>, RunnerError>>()?;

                Ok(WorkflowRunSearchResult {
                    items,
                    total: total.max(0) as usize,
                })
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

        let is_completed = matches!(
            summary.status,
            crate::core::runtime::WorkflowRunStatus::Completed
                | crate::core::runtime::WorkflowRunStatus::Failed
                | crate::core::runtime::WorkflowRunStatus::Terminated
        );

        let pool = self.pool.clone();
        let run_id = summary.run_id.clone();
        let workflow_key = summary.workflow_key.clone();
        let workflow_version = summary.workflow_version as i32;
        let current_node_id = summary.current_node_id.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query(
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
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to update summary: {}", e)))?;

                if is_completed {
                    sqlx::query(
                        r#"
                        DELETE FROM workflow_snapshots WHERE run_id = $1
                        "#,
                    )
                    .bind(&run_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| RunnerError::Store(format!("Failed to delete snapshot: {}", e)))?;
                }

                Ok(())
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::core::runtime::NextSignal;

    use super::deserialize_optional_json_field;

    #[test]
    fn treats_json_null_as_missing_optional_field() {
        let value = Some(serde_json::Value::Null);

        let parsed = deserialize_optional_json_field::<NextSignal>(value, "last_signal")
            .expect("json null should be treated as missing");

        assert!(parsed.is_none());
    }

    #[test]
    fn deserializes_optional_field_when_payload_exists() {
        let value = Some(json!({
            "type": "event",
            "payload": {
                "id": "evt-1"
            }
        }));

        let parsed = deserialize_optional_json_field::<NextSignal>(value, "last_signal")
            .expect("valid payload should deserialize");

        assert_eq!(parsed.expect("signal should be present").signal_type, "event");
    }
}
