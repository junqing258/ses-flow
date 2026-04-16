use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Row, postgres::PgPool};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::core::definition::{WorkflowDefinition, deserialize_workflow_definition};
use crate::error::RunnerError;

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowSummaryRecord {
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
    pub name: String,
    pub status: String,
    pub version: String,
    #[serde(rename = "runningRunCount")]
    pub running_run_count: u32,
    #[serde(rename = "ownerName", skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[serde(rename = "publishedAt", skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowDetailRecord {
    #[serde(flatten)]
    pub summary: WorkflowSummaryRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<Value>,
    pub workflow: WorkflowDefinition,
}

#[derive(Debug, Clone)]
pub struct StoredWorkflowDefinition {
    pub id: String,
    pub workspace_id: String,
    pub definition: WorkflowDefinition,
    pub editor_document: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub trait WorkflowCatalogStore: Send + Sync {
    fn save_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), RunnerError>;
    fn load_workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceRecord>, RunnerError>;
    fn load_all_workspaces(&self) -> Result<Vec<WorkspaceRecord>, RunnerError>;

    fn save_workflow(&self, workflow: &StoredWorkflowDefinition) -> Result<(), RunnerError>;
    fn load_workflow(&self, workflow_id: &str) -> Result<Option<StoredWorkflowDefinition>, RunnerError>;
    fn load_all_workflows(&self) -> Result<Vec<StoredWorkflowDefinition>, RunnerError>;
    fn load_workflows_by_workspace(&self, workspace_id: &str) -> Result<Vec<StoredWorkflowDefinition>, RunnerError>;
    fn delete_workflow(&self, workflow_id: &str) -> Result<(), RunnerError>;
}

pub struct PostgresCatalogStore {
    pool: PgPool,
    cache: Arc<Mutex<CatalogCache>>,
}

#[derive(Default)]
struct CatalogCache {
    workspaces: HashMap<String, WorkspaceRecord>,
    workflows: HashMap<String, StoredWorkflowDefinition>,
}

impl PostgresCatalogStore {
    pub async fn new(pool: PgPool) -> Result<Self, RunnerError> {
        let store = Self {
            pool,
            cache: Arc::new(Mutex::new(CatalogCache::default())),
        };
        store.init_schema().await?;
        store.refresh_cache().await?;
        Ok(store)
    }

    async fn init_schema(&self) -> Result<(), RunnerError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workspaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create workspaces table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_definitions (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                workflow_key TEXT NOT NULL,
                workflow_version INTEGER NOT NULL,
                name TEXT,
                definition JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT fk_workspace FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create workflow_definitions table: {}", e)))?;

        sqlx::query(
            r#"
            ALTER TABLE workflow_definitions
            ADD COLUMN IF NOT EXISTS editor_document JSONB
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to alter workflow_definitions table: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_definitions_workspace_id ON workflow_definitions(workspace_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_workflow_definitions_key_version ON workflow_definitions(workflow_key, workflow_version)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    async fn refresh_cache(&self) -> Result<(), RunnerError> {
        let workspaces = self.load_all_workspaces_from_db().await?;
        let workflows = self.load_all_workflows_from_db().await?;

        let mut cache = self
            .cache
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog cache lock".to_string()))?;

        cache.workspaces.clear();
        for workspace in workspaces {
            cache.workspaces.insert(workspace.id.clone(), workspace);
        }

        cache.workflows.clear();
        for workflow in workflows {
            cache.workflows.insert(workflow.id.clone(), workflow);
        }

        Ok(())
    }

    async fn load_all_workspaces_from_db(&self) -> Result<Vec<WorkspaceRecord>, RunnerError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name FROM workspaces ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to load workspaces: {}", e)))?;

        rows.into_iter()
            .map(|row| {
                Ok(WorkspaceRecord {
                    id: row
                        .try_get("id")
                        .map_err(|e| RunnerError::Store(format!("Failed to get id: {}", e)))?,
                    name: row
                        .try_get("name")
                        .map_err(|e| RunnerError::Store(format!("Failed to get name: {}", e)))?,
                })
            })
            .collect()
    }

    async fn load_all_workflows_from_db(&self) -> Result<Vec<StoredWorkflowDefinition>, RunnerError> {
        let rows = sqlx::query(
            r#"
            SELECT id, workspace_id, workflow_key, workflow_version, name, definition, editor_document, created_at, updated_at
            FROM workflow_definitions
            ORDER BY updated_at DESC, created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RunnerError::Store(format!("Failed to load workflows: {}", e)))?;

        rows.into_iter()
            .map(|row| {
                let id: String = row
                    .try_get("id")
                    .map_err(|e| RunnerError::Store(format!("Failed to get id: {}", e)))?;
                let workspace_id: String = row
                    .try_get("workspace_id")
                    .map_err(|e| RunnerError::Store(format!("Failed to get workspace_id: {}", e)))?;
                let definition_json: serde_json::Value = row
                    .try_get("definition")
                    .map_err(|e| RunnerError::Store(format!("Failed to get definition: {}", e)))?;
                let editor_document: Option<serde_json::Value> = row
                    .try_get("editor_document")
                    .map_err(|e| RunnerError::Store(format!("Failed to get editor_document: {}", e)))?;
                let created_at: DateTime<Utc> = row
                    .try_get("created_at")
                    .map_err(|e| RunnerError::Store(format!("Failed to get created_at: {}", e)))?;
                let updated_at: DateTime<Utc> = row
                    .try_get("updated_at")
                    .map_err(|e| RunnerError::Store(format!("Failed to get updated_at: {}", e)))?;
                let definition = deserialize_workflow_definition(definition_json)
                    .map_err(|e| RunnerError::Store(format!("Failed to deserialize workflow definition: {}", e)))?;

                Ok(StoredWorkflowDefinition {
                    id,
                    workspace_id,
                    definition,
                    editor_document,
                    created_at,
                    updated_at,
                })
            })
            .collect()
    }
}

impl WorkflowCatalogStore for PostgresCatalogStore {
    fn save_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), RunnerError> {
        let pool = self.pool.clone();
        let workspace_clone = workspace.clone();
        let cache = self.cache.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _result = sqlx::query(
                    r#"
                    INSERT INTO workspaces (id, name)
                    VALUES ($1, $2)
                    ON CONFLICT (id) DO UPDATE SET
                        name = EXCLUDED.name,
                        updated_at = NOW()
                    "#,
                )
                .bind(&workspace_clone.id)
                .bind(&workspace_clone.name)
                .execute(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to save workspace: {}", e)))?;

                // Update cache on success
                if let Ok(mut cache) = cache.lock() {
                    cache
                        .workspaces
                        .insert(workspace_clone.id.clone(), workspace_clone.clone());
                }

                Ok(())
            })
        })
    }

    fn load_workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceRecord>, RunnerError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog cache lock".to_string()))?;
        Ok(cache.workspaces.get(workspace_id).cloned())
    }

    fn load_all_workspaces(&self) -> Result<Vec<WorkspaceRecord>, RunnerError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog cache lock".to_string()))?;
        Ok(cache.workspaces.values().cloned().collect())
    }

    fn save_workflow(&self, workflow: &StoredWorkflowDefinition) -> Result<(), RunnerError> {
        let pool = self.pool.clone();
        let workflow_clone = workflow.clone();
        let definition_value = serde_json::to_value(&workflow.definition)
            .map_err(|e| RunnerError::Store(format!("Failed to serialize workflow definition: {}", e)))?;
        let editor_document_value = workflow.editor_document.clone();
        let cache = self.cache.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _result = sqlx::query(
                    r#"
                    INSERT INTO workflow_definitions (id, workspace_id, workflow_key, workflow_version, name, definition, editor_document, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (id) DO UPDATE SET
                        workspace_id = EXCLUDED.workspace_id,
                        workflow_key = EXCLUDED.workflow_key,
                        workflow_version = EXCLUDED.workflow_version,
                        name = EXCLUDED.name,
                        definition = EXCLUDED.definition,
                        editor_document = EXCLUDED.editor_document,
                        updated_at = NOW()
                    "#,
                )
                .bind(&workflow_clone.id)
                .bind(&workflow_clone.workspace_id)
                .bind(&workflow_clone.definition.meta.key)
                .bind(workflow_clone.definition.meta.version as i32)
                .bind(&workflow_clone.definition.meta.name)
                .bind(definition_value)
                .bind(editor_document_value)
                .bind(workflow_clone.created_at)
                .bind(workflow_clone.updated_at)
                .execute(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to save workflow: {}", e)))?;

                // Update cache on success
                if let Ok(mut cache) = cache.lock() {
                    cache.workflows.insert(workflow_clone.id.clone(), workflow_clone.clone());
                }

                Ok(())
            })
        })
    }

    fn load_workflow(&self, workflow_id: &str) -> Result<Option<StoredWorkflowDefinition>, RunnerError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog cache lock".to_string()))?;
        Ok(cache.workflows.get(workflow_id).cloned())
    }

    fn load_all_workflows(&self) -> Result<Vec<StoredWorkflowDefinition>, RunnerError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog cache lock".to_string()))?;
        Ok(cache.workflows.values().cloned().collect())
    }

    fn load_workflows_by_workspace(&self, workspace_id: &str) -> Result<Vec<StoredWorkflowDefinition>, RunnerError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog cache lock".to_string()))?;
        Ok(cache
            .workflows
            .values()
            .filter(|w| w.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    fn delete_workflow(&self, workflow_id: &str) -> Result<(), RunnerError> {
        let pool = self.pool.clone();
        let workflow_id = workflow_id.to_string();
        let cache = self.cache.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                sqlx::query(
                    r#"
                    DELETE FROM workflow_definitions WHERE id = $1
                    "#,
                )
                .bind(&workflow_id)
                .execute(&pool)
                .await
                .map_err(|e| RunnerError::Store(format!("Failed to delete workflow: {}", e)))?;

                // Remove the workflow from the in-memory cache only after the
                // database delete has completed successfully.
                if let Ok(mut cache) = cache.lock() {
                    cache.workflows.remove(&workflow_id);
                }

                Ok(())
            })
        })
    }
}
