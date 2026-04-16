use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use tracing::info;

use super::run_service::{self, ExecuteWorkflowRequest, ResumeWorkflowRequest, WorkflowExecutionAccepted};
use crate::server::{ApiError, ApiState, WorkflowEventStream};
use runner::core::runtime::WorkflowRunSummary;

pub async fn execute_workflow(
    State(state): State<ApiState>,
    Path(workflow_id): Path<String>,
    Json(request): Json<ExecuteWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowExecutionAccepted>), ApiError> {
    let (status, response) = run_service::execute_workflow(&state, workflow_id, request).await?;
    Ok((status, Json(response)))
}

pub async fn resume_workflow(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
    Json(request): Json<ResumeWorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowExecutionAccepted>), ApiError> {
    let (status, response) = run_service::resume_workflow(&state, run_id, request).await?;
    Ok((status, Json(response)))
}

pub async fn get_run_summary(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<Json<WorkflowRunSummary>, ApiError> {
    Ok(Json(run_service::get_run_summary(&state, &run_id)?))
}

pub async fn subscribe_run_events(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<WorkflowEventStream, ApiError> {
    run_service::subscribe_run_events(&state, &run_id)
}

pub async fn terminate_workflow(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Result<Json<WorkflowRunSummary>, ApiError> {
    info!(run_id = %run_id, "terminating workflow run");
    Ok(Json(run_service::terminate_workflow(&state, &run_id)?))
}
