use runner::app::WorkflowRegistration;
use runner::core::definition::WorkflowDefinition;
use runner::store::{WorkflowDetailRecord, WorkflowRunRecord, WorkflowSummaryRecord};
use serde::Deserialize;
use serde_json::Value;
use tracing::info;

use crate::modules::{ApiError, ApiState, WorkflowEventStream, into_sse};

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
    Ok(state.app.list_workflows()?)
}

pub fn subscribe_workflows_events(state: &ApiState) -> WorkflowEventStream {
    into_sse(state.app.subscribe_workflows_events())
}

pub fn register_workflow(
    state: &ApiState,
    request: UploadWorkflowRequest,
) -> Result<WorkflowRegistration, ApiError> {
    info!(
        workspace_id = request.workspace_id.as_deref().unwrap_or("default"),
        workflow_key = request.workflow.meta.key,
        workflow_version = request.workflow.meta.version,
        message = "registering workflow",
    );
    let registration = state.app.register_workflow(
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
    Ok(state.app.get_workflow(workflow_id)?)
}

pub fn subscribe_workflow_events(
    state: &ApiState,
    workflow_id: &str,
) -> Result<WorkflowEventStream, ApiError> {
    state.app.get_workflow(workflow_id)?;
    Ok(into_sse(state.app.subscribe_workflow_events(workflow_id)))
}

pub fn list_workflow_runs(
    state: &ApiState,
    workflow_id: &str,
) -> Result<Vec<WorkflowRunRecord>, ApiError> {
    Ok(state.app.list_workflow_runs(workflow_id)?)
}

pub fn refresh_catalog(state: &ApiState) -> Result<RefreshCatalogResponse, ApiError> {
    state.app.refresh_catalog()?;
    Ok(RefreshCatalogResponse { status: "ok" })
}
