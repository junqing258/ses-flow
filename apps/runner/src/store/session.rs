use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Row, postgres::PgPool};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::core::definition::{WorkflowDefinition, deserialize_workflow_definition};
use crate::error::RunnerError;

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowEditSessionRecord {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    #[serde(rename = "workflowId", skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    pub workflow: WorkflowDefinition,
    #[serde(rename = "editorDocument", skip_serializing_if = "Option::is_none")]
    pub editor_document: Option<Value>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

pub trait WorkflowEditSessionStore: Send + Sync {
    fn save_session(&self, session: &WorkflowEditSessionRecord) -> Result<(), RunnerError>;
    fn load_session(&self, session_id: &str) -> Result<Option<WorkflowEditSessionRecord>, RunnerError>;
}

#[derive(Default)]
pub struct InMemoryEditSessionStore {
    sessions: Arc<Mutex<HashMap<String, WorkflowEditSessionRecord>>>,
}

impl InMemoryEditSessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl WorkflowEditSessionStore for InMemoryEditSessionStore {
    fn save_session(&self, session: &WorkflowEditSessionRecord) -> Result<(), RunnerError> {
        let mut state = self
            .sessions
            .lock()
            .map_err(|_| RunnerError::Store("Failed to lock edit session store".to_string()))?;
        state.insert(session.session_id.clone(), session.clone());
        Ok(())
    }

    fn load_session(&self, session_id: &str) -> Result<Option<WorkflowEditSessionRecord>, RunnerError> {
        let state = self
            .sessions
            .lock()
            .map_err(|_| RunnerError::Store("Failed to lock edit session store".to_string()))?;
        Ok(state.get(session_id).cloned())
    }
}

pub struct PostgresEditSessionStore {
    pool: PgPool,
}

impl PostgresEditSessionStore {
    pub async fn new(pool: PgPool) -> Result<Self, RunnerError> {
        let store = Self { pool };
        store.init_schema().await?;
        Ok(store)
    }

    async fn init_schema(&self) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_edit_sessions (
                session_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                workflow_id TEXT,
                workflow_definition JSONB NOT NULL,
                editor_document JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create workflow_edit_sessions table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_edit_sessions_workflow_id
            ON workflow_edit_sessions(workflow_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create workflow_edit_sessions index: {}", e)))?;

        Ok(())
    }
}

impl WorkflowEditSessionStore for PostgresEditSessionStore {
    fn save_session(&self, session: &WorkflowEditSessionRecord) -> Result<(), RunnerError> {
        let pool = self.pool.clone();
        let session = session.clone();
        let workflow_value = serde_json::to_value(&session.workflow)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize workflow edit session definition: {}", e)))?;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query(
                    r#"
                    INSERT INTO workflow_edit_sessions (
                        session_id,
                        workspace_id,
                        workflow_id,
                        workflow_definition,
                        editor_document,
                        created_at,
                        updated_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    ON CONFLICT (session_id) DO UPDATE SET
                        workspace_id = EXCLUDED.workspace_id,
                        workflow_id = EXCLUDED.workflow_id,
                        workflow_definition = EXCLUDED.workflow_definition,
                        editor_document = EXCLUDED.editor_document,
                        updated_at = EXCLUDED.updated_at
                    "#,
                )
                .bind(&session.session_id)
                .bind(&session.workspace_id)
                .bind(&session.workflow_id)
                .bind(workflow_value)
                .bind(session.editor_document.clone())
                .bind(session.created_at)
                .bind(session.updated_at)
                .execute(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to save workflow edit session: {}", e)))?;

                Ok(())
            })
        })
    }

    fn load_session(&self, session_id: &str) -> Result<Option<WorkflowEditSessionRecord>, RunnerError> {
        let pool = self.pool.clone();
        let session_id = session_id.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let row = sqlx::query(
                    r#"
                    SELECT
                        session_id,
                        workspace_id,
                        workflow_id,
                        workflow_definition,
                        editor_document,
                        created_at,
                        updated_at
                    FROM workflow_edit_sessions
                    WHERE session_id = $1
                    "#,
                )
                .bind(&session_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to load workflow edit session: {}", e)))?;

                row.map(|row| {
                    let workflow_value: Value = row
                        .try_get("workflow_definition")
                        .map_err(|e| RunnerError::Store(format!("Failed to get workflow_definition: {}", e)))?;
                    let workflow = deserialize_workflow_definition(workflow_value)?;

                    Ok(WorkflowEditSessionRecord {
                        session_id: row
                            .try_get("session_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get session_id: {}", e)))?,
                        workspace_id: row
                            .try_get("workspace_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workspace_id: {}", e)))?,
                        workflow_id: row
                            .try_get("workflow_id")
                            .map_err(|e| RunnerError::Store(format!("Failed to get workflow_id: {}", e)))?,
                        workflow,
                        editor_document: row
                            .try_get("editor_document")
                            .map_err(|e| RunnerError::Store(format!("Failed to get editor_document: {}", e)))?,
                        created_at: row
                            .try_get("created_at")
                            .map_err(|e| RunnerError::Store(format!("Failed to get created_at: {}", e)))?,
                        updated_at: row
                            .try_get("updated_at")
                            .map_err(|e| RunnerError::Store(format!("Failed to get updated_at: {}", e)))?,
                    })
                })
                .transpose()
            })
        })
    }
}
