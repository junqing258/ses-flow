use runner::app::EditSessionDraftOperation;
use runner::core::definition::WorkflowDefinition;
use runner::store::WorkflowEditSessionRecord;
use serde::Deserialize;
use serde_json::Value;

use crate::modules::{ApiError, ApiState, WorkflowEventStream, into_sse};

fn default_true() -> bool {
    true
}

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

#[derive(Debug, Deserialize)]
pub struct EditSessionPatchRequest {
    #[serde(rename = "workflowId", default)]
    pub workflow_id: Option<String>,
    pub operations: Vec<EditSessionDraftOperation>,
}

#[derive(Debug, Default, Deserialize)]
pub struct EditSessionGetRequest {
    #[serde(rename = "includeEditorDocument", default = "default_true")]
    pub include_editor_document: bool,
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
    request: EditSessionGetRequest,
) -> Result<WorkflowEditSessionRecord, ApiError> {
    let mut session = state.app.get_edit_session(session_id)?;

    if !request.include_editor_document {
        session.editor_document = None;
    }

    Ok(session)
}

pub fn subscribe_edit_session_events(state: &ApiState, session_id: &str) -> Result<WorkflowEventStream, ApiError> {
    state.app.get_edit_session(session_id)?;
    Ok(into_sse(state.app.subscribe_edit_session_events(session_id)))
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

pub fn patch_edit_session(
    state: &ApiState,
    session_id: &str,
    request: EditSessionPatchRequest,
) -> Result<WorkflowEditSessionRecord, ApiError> {
    Ok(state
        .app
        .apply_edit_session_operations(session_id, request.workflow_id, request.operations)?)
}
