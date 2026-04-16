use axum::http::StatusCode;
use runner::core::runtime::{RunEnvironment, WorkflowRunSummary};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

use crate::server::{ApiError, ApiState, RUNNER_API_BASE_PATH, WorkflowEventStream, into_sse};

#[derive(Debug, Deserialize)]
pub struct ExecuteWorkflowRequest {
    #[serde(default)]
    pub trigger: Option<Value>,
    #[serde(default)]
    pub env: Option<RunEnvironment>,
}

#[derive(Debug, Deserialize)]
pub struct ResumeWorkflowRequest {
    pub event: Value,
}

#[derive(Debug, Serialize)]
pub struct WorkflowExecutionAccepted {
    #[serde(rename = "workflowId", skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub status: &'static str,
    #[serde(rename = "statusUrl")]
    pub status_url: String,
}

pub async fn execute_workflow(
    state: &ApiState,
    workflow_id: String,
    request: ExecuteWorkflowRequest,
) -> Result<(StatusCode, WorkflowExecutionAccepted), ApiError> {
    info!(workflow_id = %workflow_id, "starting workflow run");
    let trigger = request.trigger.unwrap_or_else(default_trigger);
    let env = request.env.unwrap_or_default();
    let summary = state.app.start_workflow(&workflow_id, trigger, env).await?;
    info!(workflow_id = %workflow_id, run_id = %summary.run_id, "workflow run accepted");

    Ok((
        StatusCode::ACCEPTED,
        WorkflowExecutionAccepted {
            workflow_id: Some(workflow_id),
            run_id: summary.run_id.clone(),
            status: "accepted",
            status_url: format!("{RUNNER_API_BASE_PATH}/runs/{}", summary.run_id),
        },
    ))
}

pub async fn resume_workflow(
    state: &ApiState,
    run_id: String,
    request: ResumeWorkflowRequest,
) -> Result<(StatusCode, WorkflowExecutionAccepted), ApiError> {
    info!(run_id = %run_id, "resuming workflow run");
    let summary = state.app.resume_workflow(&run_id, request.event).await?;
    info!(run_id = %summary.run_id, "workflow resume accepted");

    Ok((
        StatusCode::ACCEPTED,
        WorkflowExecutionAccepted {
            workflow_id: None,
            run_id: summary.run_id.clone(),
            status: "accepted",
            status_url: format!("{RUNNER_API_BASE_PATH}/runs/{}", summary.run_id),
        },
    ))
}

pub fn get_run_summary(state: &ApiState, run_id: &str) -> Result<WorkflowRunSummary, ApiError> {
    state
        .app
        .get_summary(run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))
}

pub fn subscribe_run_events(state: &ApiState, run_id: &str) -> Result<WorkflowEventStream, ApiError> {
    state
        .app
        .get_summary(run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))?;
    Ok(into_sse(state.app.subscribe_run_events(run_id)))
}

pub fn terminate_workflow(state: &ApiState, run_id: &str) -> Result<WorkflowRunSummary, ApiError> {
    Ok(state.app.terminate_workflow(run_id)?)
}

fn default_trigger() -> Value {
    json!({
        "headers": {
            "requestId": "req-demo-1"
        },
        "body": {
            "orderNo": "SO-DEMO-1",
            "bizType": "auto_sort"
        }
    })
}
