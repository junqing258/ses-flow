use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;

use crate::error::RunnerError;

use super::catalog::{StoredWorkflowDefinition, WorkflowCatalogStore, WorkspaceRecord};

// 基于内存的 catalog 实现，主要用于测试场景，以及未提供数据库 catalog
// 的轻量 server 装配方式。它和 PostgreSQL 版承担相同的工作流目录职责，
// 区别只是所有状态都保存在当前进程内。
pub struct InMemoryCatalogStore {
    state: Arc<Mutex<InMemoryCatalogState>>,
}

#[derive(Default)]
struct InMemoryCatalogState {
    workspaces: HashMap<String, WorkspaceRecord>,
    workflows: HashMap<String, StoredWorkflowDefinition>,
}

impl InMemoryCatalogStore {
    pub fn new() -> Self {
        let store = Self::default();
        // 与 API 层默认 workspace 约定保持一致，这样本地和测试场景下无需
        // 额外初始化，就可以直接注册 workflow。
        let _ = store.save_workspace(&WorkspaceRecord {
            id: "default".to_string(),
            name: "Default".to_string(),
        });
        store
    }
}

impl Default for InMemoryCatalogStore {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(InMemoryCatalogState::default())),
        }
    }
}

impl WorkflowCatalogStore for InMemoryCatalogStore {
    fn save_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        state.workspaces.insert(workspace.id.clone(), workspace.clone());
        Ok(())
    }

    fn load_workspace(&self, workspace_id: &str) -> Result<Option<WorkspaceRecord>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        Ok(state.workspaces.get(workspace_id).cloned())
    }

    fn load_all_workspaces(&self) -> Result<Vec<WorkspaceRecord>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        Ok(state.workspaces.values().cloned().collect())
    }

    fn save_workflow(&self, workflow: &StoredWorkflowDefinition) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        let mut workflow_record = workflow.clone();

        if let Some(existing) = state.workflows.get(&workflow.id) {
            workflow_record.created_at = existing.created_at;
        }

        workflow_record.updated_at = Utc::now();
        state.workflows.insert(workflow.id.clone(), workflow_record);
        Ok(())
    }

    fn load_workflow(&self, workflow_id: &str) -> Result<Option<StoredWorkflowDefinition>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        Ok(state.workflows.get(workflow_id).cloned())
    }

    fn load_all_workflows(&self) -> Result<Vec<StoredWorkflowDefinition>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        Ok(state.workflows.values().cloned().collect())
    }

    fn load_workflows_by_workspace(&self, workspace_id: &str) -> Result<Vec<StoredWorkflowDefinition>, RunnerError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        Ok(state
            .workflows
            .values()
            .filter(|w| w.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    fn delete_workflow(&self, workflow_id: &str) -> Result<(), RunnerError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RunnerError::Store("Failed to acquire catalog lock".to_string()))?;
        state.workflows.remove(workflow_id);
        Ok(())
    }

    fn refresh(&self) -> Result<(), RunnerError> {
        Ok(())
    }
}
