use axum::Json;
use axum::extract::{Path, Query, State};

use super::edit_session_service::{self, EditSessionGetRequest, EditSessionPatchRequest, EditSessionUpsertRequest};
use crate::modules::{ApiError, ApiState, WorkflowEventStream};
use runner::store::WorkflowEditSessionRecord;

pub async fn create_edit_session(
    State(state): State<ApiState>,
    Json(request): Json<EditSessionUpsertRequest>,
) -> Result<Json<WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(edit_session_service::create_edit_session(&state, request)?))
}

pub async fn get_edit_session(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
    Query(request): Query<EditSessionGetRequest>,
) -> Result<Json<WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(edit_session_service::get_edit_session(
        &state,
        &session_id,
        request,
    )?))
}

pub async fn subscribe_edit_session_events(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
) -> Result<WorkflowEventStream, ApiError> {
    edit_session_service::subscribe_edit_session_events(&state, &session_id)
}

pub async fn update_edit_session(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
    Json(request): Json<EditSessionUpsertRequest>,
) -> Result<Json<WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(edit_session_service::update_edit_session(
        &state,
        &session_id,
        request,
    )?))
}

pub async fn patch_edit_session(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
    Json(request): Json<EditSessionPatchRequest>,
) -> Result<Json<WorkflowEditSessionRecord>, ApiError> {
    Ok(Json(edit_session_service::patch_edit_session(
        &state,
        &session_id,
        request,
    )?))
}
