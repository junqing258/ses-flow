use axum::Json;
use axum::extract::{Path, State};
use tracing::debug;

use super::workflow_service::{self, UploadWorkflowRequest};
use crate::server::{ApiError, ApiState, WorkflowEventStream, WorkflowRegistration};
use crate::store::{WorkflowDetailRecord, WorkflowRunRecord, WorkflowSummaryRecord};

pub async fn list_workflows(State(state): State<ApiState>) -> Result<Json<Vec<WorkflowSummaryRecord>>, ApiError> {
    debug!("listing workflows");
    Ok(Json(workflow_service::list_workflows(&state)?))
}

pub async fn subscribe_workflows_events(State(state): State<ApiState>) -> Result<WorkflowEventStream, ApiError> {
    Ok(workflow_service::subscribe_workflows_events(&state))
}

pub async fn upload_workflow(
    State(state): State<ApiState>,
    Json(request): Json<UploadWorkflowRequest>,
) -> Result<Json<WorkflowRegistration>, ApiError> {
    Ok(Json(workflow_service::register_workflow(&state, request)?))
}

pub async fn get_workflow(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
) -> Result<Json<WorkflowDetailRecord>, ApiError> {
    debug!(workflow_id = %workflow_id, "fetching workflow");
    Ok(Json(workflow_service::get_workflow(&state, &workflow_id)?))
}

pub async fn subscribe_workflow_events(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
) -> Result<WorkflowEventStream, ApiError> {
    workflow_service::subscribe_workflow_events(&state, &workflow_id)
}

pub async fn list_workflow_runs(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
) -> Result<Json<Vec<WorkflowRunRecord>>, ApiError> {
    debug!(workflow_id = %workflow_id, "listing workflow runs");
    Ok(Json(workflow_service::list_workflow_runs(&state, &workflow_id)?))
}
