use serde::Deserialize;
use serde_json::Value;

use crate::core::definition::WorkflowDefinition;
use crate::server::{ApiError, ApiState, WorkflowEventStream};
use crate::store::WorkflowEditSessionRecord;

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
    Ok(state.server.create_edit_session(
        request.workspace_id,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?)
}

pub fn get_edit_session(state: &ApiState, session_id: &str) -> Result<WorkflowEditSessionRecord, ApiError> {
    Ok(state.server.get_edit_session(session_id)?)
}

pub fn subscribe_edit_session_events(state: &ApiState, session_id: &str) -> Result<WorkflowEventStream, ApiError> {
    state.server.get_edit_session(session_id)?;
    Ok(state.server.subscribe_edit_session_events(session_id))
}

pub fn update_edit_session(
    state: &ApiState,
    session_id: &str,
    request: EditSessionUpsertRequest,
) -> Result<WorkflowEditSessionRecord, ApiError> {
    Ok(state.server.update_edit_session(
        session_id,
        request.workflow_id,
        request.workflow,
        request.editor_document,
    )?)
}
