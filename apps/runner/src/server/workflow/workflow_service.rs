use serde::Deserialize;
use serde_json::Value;
use tracing::info;

use crate::core::definition::WorkflowDefinition;
use crate::server::{ApiError, ApiState, WorkflowEventStream, WorkflowRegistration};
use crate::store::{WorkflowDetailRecord, WorkflowRunRecord, WorkflowSummaryRecord};

#[derive(Debug, serde::Serialize)]
pub struct RefreshCatalogResponse {
    pub status: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct UploadWorkflowRequest {
    #[serde(rename = "workspaceId", default)]
    pub workspace_id: Option<String>,
    #[serde(rename = "workspaceName", default)]
    pub workspace_name: Option<String>,
    #[serde(rename = "workflowId", default)]
    pub workflow_id: Option<String>,
    #[serde(rename = "editorDocument", default)]
    pub editor_document: Option<Value>,
    pub workflow: WorkflowDefinition,
}

pub fn list_workflows(state: &ApiState) -> Result<Vec<WorkflowSummaryRecord>, ApiError> {
    Ok(state.server.list_workflows()?)
}

pub fn subscribe_workflows_events(state: &ApiState) -> WorkflowEventStream {
    state.server.subscribe_workflows_events()
}

pub fn register_workflow(state: &ApiState, request: UploadWorkflowRequest) -> Result<WorkflowRegistration, ApiError> {
    info!(
        workspace_id = request.workspace_id.as_deref().unwrap_or("default"),
        workflow_key = request.workflow.meta.key,
        workflow_version = request.workflow.meta.version,
        message = "registering workflow",
    );
    let registration = state.server.register_workflow(
        request.workspace_id,
        request.workspace_name,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?;
    info!(
        workflow_id = %registration.workflow_id,
        workspace_id = %registration.workspace_id,
        workflow_key = %registration.workflow_key,
        workflow_version = registration.workflow_version,
        "workflow registered",
    );
    Ok(registration)
}

pub fn get_workflow(state: &ApiState, workflow_id: &str) -> Result<WorkflowDetailRecord, ApiError> {
    Ok(state.server.get_workflow(workflow_id)?)
}

pub fn subscribe_workflow_events(state: &ApiState, workflow_id: &str) -> Result<WorkflowEventStream, ApiError> {
    state.server.get_workflow(workflow_id)?;
    Ok(state.server.subscribe_workflow_events(workflow_id))
}

pub fn list_workflow_runs(state: &ApiState, workflow_id: &str) -> Result<Vec<WorkflowRunRecord>, ApiError> {
    Ok(state.server.list_workflow_runs(workflow_id)?)
}

pub fn refresh_catalog(state: &ApiState) -> Result<RefreshCatalogResponse, ApiError> {
    state.server.refresh_catalog()?;
    Ok(RefreshCatalogResponse { status: "ok" })
}
