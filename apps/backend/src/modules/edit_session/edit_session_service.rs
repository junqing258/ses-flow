use runner::core::definition::WorkflowDefinition;
use runner::store::WorkflowEditSessionRecord;
use serde::Deserialize;
use serde_json::Value;

use crate::modules::{ApiError, ApiState, WorkflowEventStream, into_sse};

#[derive(Debug, Deserialize)]
pub struct EditSessionUpsertRequest {
    #[serde(rename = "workspaceId", default)]
    pub workspace_id: Option<String>,
    #[serde(rename = "workflowId", default)]
    pub workflow_id: Option<String>,
    #[serde(rename = "editorDocument", default)]
    pub editor_document: Option<Value>,
    pub workflow: WorkflowDefinition,
}

pub fn create_edit_session(
    state: &ApiState,
    request: EditSessionUpsertRequest,
) -> Result<WorkflowEditSessionRecord, ApiError> {
    Ok(state.app.create_edit_session(
        request.workspace_id,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?)
}

pub fn get_edit_session(
    state: &ApiState,
    session_id: &str,
) -> Result<WorkflowEditSessionRecord, ApiError> {
    Ok(state.app.get_edit_session(session_id)?)
}

pub fn subscribe_edit_session_events(
    state: &ApiState,
    session_id: &str,
) -> Result<WorkflowEventStream, ApiError> {
    state.app.get_edit_session(session_id)?;
    Ok(into_sse(
        state.app.subscribe_edit_session_events(session_id),
    ))
}

pub fn update_edit_session(
    state: &ApiState,
    session_id: &str,
    request: EditSessionUpsertRequest,
) -> Result<WorkflowEditSessionRecord, ApiError> {
    Ok(state.app.update_edit_session(
        session_id,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?)
}
