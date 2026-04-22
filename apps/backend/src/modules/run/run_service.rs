use axum::http::StatusCode;
use runner::core::runtime::{NodeExecutionRecord, RunEnvironment, WorkflowRunStatus, WorkflowRunSummary};
use runner::store::WorkflowRunSearchQuery;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::info;

use crate::modules::{ApiError, ApiState, RUNNER_API_BASE_PATH, WorkflowEventStream, into_sse};

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

#[derive(Debug, Deserialize)]
pub struct RunSearchRequest {
    #[serde(rename = "runId", default)]
    pub run_id: Option<String>,
    #[serde(rename = "requestId", default)]
    pub request_id: Option<String>,
    #[serde(rename = "orderNo", default)]
    pub order_no: Option<String>,
    #[serde(rename = "waveNo", default)]
    pub wave_no: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(rename = "pageSize", default = "default_page_size")]
    pub page_size: u32,
}

#[derive(Debug, Deserialize)]
pub struct ManualPatchRequest {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    pub note: String,
    pub operator: String,
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

#[derive(Debug, Serialize)]
pub struct WorkflowRunSearchResponse {
    pub items: Vec<WorkflowRunSearchItem>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct WorkflowRunSearchItem {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    pub status: WorkflowRunStatus,
    #[serde(rename = "orderNo", skip_serializing_if = "Option::is_none")]
    pub order_no: Option<String>,
    #[serde(rename = "waveNo", skip_serializing_if = "Option::is_none")]
    pub wave_no: Option<String>,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "endedAt", skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowRunSummaryResponse {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "workflowKey")]
    pub workflow_key: String,
    #[serde(rename = "workflowVersion")]
    pub workflow_version: u32,
    pub status: WorkflowRunStatus,
    #[serde(rename = "currentNodeId", skip_serializing_if = "Option::is_none")]
    pub current_node_id: Option<String>,
    pub state: Value,
    pub timeline: Vec<WorkflowRunTimelineItemResponse>,
    #[serde(rename = "lastSignal", skip_serializing_if = "Option::is_none")]
    pub last_signal: Option<runner::core::runtime::NextSignal>,
    #[serde(rename = "resumeState", skip_serializing_if = "Option::is_none")]
    pub resume_state: Option<runner::core::runtime::WorkflowRunSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowRunTimelineItemResponse {
    #[serde(rename = "nodeId")]
    pub node_id: String,
    #[serde(rename = "nodeType")]
    pub node_type: runner::core::definition::NodeType,
    pub status: runner::core::runtime::ExecutionStatus,
    pub input: Value,
    pub output: Value,
    #[serde(rename = "statePatch")]
    pub state_patch: Value,
    #[serde(rename = "branchKey", skip_serializing_if = "Option::is_none")]
    pub branch_key: Option<String>,
    #[serde(rename = "startedAt", skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(rename = "endedAt", skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(rename = "inputSummary", skip_serializing_if = "Option::is_none")]
    pub input_summary: Option<String>,
    #[serde(rename = "outputSummary", skip_serializing_if = "Option::is_none")]
    pub output_summary: Option<String>,
    #[serde(rename = "errorCode", skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(rename = "errorDetail", skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
    #[serde(rename = "recoveryHint", skip_serializing_if = "Option::is_none")]
    pub recovery_hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub logs: Vec<runner::core::runtime::NodeLogRecord>,
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

pub fn get_run_summary(state: &ApiState, run_id: &str) -> Result<WorkflowRunSummaryResponse, ApiError> {
    let summary = state
        .app
        .get_summary(run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))?;
    Ok(to_summary_response(summary))
}

pub fn search_runs(state: &ApiState, request: RunSearchRequest) -> Result<WorkflowRunSearchResponse, ApiError> {
    let result = state.app.search_runs(&WorkflowRunSearchQuery {
        run_id: normalize_optional_query(request.run_id),
        request_id: normalize_optional_query(request.request_id),
        order_no: normalize_optional_query(request.order_no),
        wave_no: normalize_optional_query(request.wave_no),
        page: request.page.max(1),
        page_size: request.page_size.clamp(1, 100),
    })?;

    Ok(WorkflowRunSearchResponse {
        total: result.total,
        items: result
            .items
            .into_iter()
            .map(|item| {
                let ended_at = is_terminal_status(&item.status).then(|| item.updated_at.to_rfc3339());
                let duration_ms = ended_at
                    .as_ref()
                    .map(|_| (item.updated_at - item.created_at).num_milliseconds().max(0));

                WorkflowRunSearchItem {
                    run_id: item.run_id,
                    workflow_key: item.workflow_key,
                    status: item.status,
                    order_no: item.order_no,
                    wave_no: item.wave_no,
                    request_id: item.request_id,
                    started_at: item.created_at.to_rfc3339(),
                    ended_at,
                    duration_ms,
                }
            })
            .collect(),
    })
}

pub fn subscribe_run_events(state: &ApiState, run_id: &str) -> Result<WorkflowEventStream, ApiError> {
    state
        .app
        .get_summary(run_id)?
        .ok_or_else(|| ApiError::NotFound(format!("workflow run not found: {run_id}")))?;
    Ok(into_sse(state.app.subscribe_run_events(run_id)))
}

pub fn terminate_workflow(state: &ApiState, run_id: &str) -> Result<WorkflowRunSummaryResponse, ApiError> {
    Ok(to_summary_response(state.app.terminate_workflow(run_id)?))
}

pub fn manual_patch_run(
    state: &ApiState,
    run_id: &str,
    request: ManualPatchRequest,
) -> Result<WorkflowRunSummaryResponse, ApiError> {
    let note = request.note.trim();
    let operator = request.operator.trim();

    if note.is_empty() {
        return Err(ApiError::BadRequest("manual patch note is required".to_string()));
    }

    if operator.is_empty() {
        return Err(ApiError::BadRequest("manual patch operator is required".to_string()));
    }

    Ok(to_summary_response(state.app.patch_run_node(
        run_id,
        &request.node_id,
        note,
        operator,
    )?))
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

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

fn normalize_optional_query(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn is_terminal_status(status: &WorkflowRunStatus) -> bool {
    matches!(
        status,
        WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Terminated
    )
}

fn to_summary_response(summary: WorkflowRunSummary) -> WorkflowRunSummaryResponse {
    WorkflowRunSummaryResponse {
        run_id: summary.run_id,
        workflow_key: summary.workflow_key,
        workflow_version: summary.workflow_version,
        status: summary.status,
        current_node_id: summary.current_node_id,
        state: summary.state,
        timeline: summary.timeline.iter().map(to_timeline_item_response).collect(),
        last_signal: summary.last_signal,
        resume_state: summary.resume_state,
    }
}

fn to_timeline_item_response(record: &NodeExecutionRecord) -> WorkflowRunTimelineItemResponse {
    WorkflowRunTimelineItemResponse {
        node_id: record.node_id.clone(),
        node_type: record.node_type,
        status: record.status.clone(),
        input: record.input.clone(),
        output: record.output.clone(),
        state_patch: record.state_patch.clone(),
        branch_key: record.branch_key.clone(),
        started_at: record.started_at.as_ref().map(|value| value.to_rfc3339()),
        ended_at: record.ended_at.as_ref().map(|value| value.to_rfc3339()),
        duration_ms: duration_ms(record),
        input_summary: summarize_value(&record.input),
        output_summary: summarize_value(&record.output),
        error_code: record.error_code.clone(),
        error_detail: record.error_detail.clone(),
        recovery_hint: record.error_code.as_deref().and_then(recovery_hint),
        logs: record.logs.clone(),
    }
}

fn duration_ms(record: &NodeExecutionRecord) -> Option<i64> {
    match (record.started_at.as_ref(), record.ended_at.as_ref()) {
        (Some(started_at), Some(ended_at)) => Some((*ended_at - *started_at).num_milliseconds().max(0)),
        _ => None,
    }
}

fn summarize_value(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let summary = match value {
        Value::String(text) => text.trim().to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Array(items) => {
            if items.is_empty() {
                "[]".to_string()
            } else {
                format!("[{} items]", items.len())
            }
        }
        Value::Object(map) => {
            let scalar_pairs = map
                .iter()
                .filter_map(|(key, value)| match value {
                    Value::String(text) => Some(format!("{key}={}", text.trim())),
                    Value::Number(number) => Some(format!("{key}={number}")),
                    Value::Bool(flag) => Some(format!("{key}={flag}")),
                    _ => None,
                })
                .take(3)
                .collect::<Vec<_>>();

            if scalar_pairs.is_empty() {
                serde_json::to_string(value).unwrap_or_else(|_| "{...}".to_string())
            } else {
                scalar_pairs.join(", ")
            }
        }
        Value::Null => String::new(),
    };

    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return None;
    }

    const MAX_SUMMARY_LENGTH: usize = 160;
    if trimmed.chars().count() <= MAX_SUMMARY_LENGTH {
        Some(trimmed.to_string())
    } else {
        Some(trimmed.chars().take(MAX_SUMMARY_LENGTH).collect::<String>() + "...")
    }
}

fn recovery_hint(code: &str) -> Option<String> {
    match code {
        "HTTP_ERROR" => Some("检查目标服务是否可用，并确认 HTTP 状态码与响应体".to_string()),
        "TIMEOUT" => Some("检查网络延迟、超时阈值和目标服务响应时间".to_string()),
        "VALIDATION_FAILED" => Some("检查节点配置、输入映射和 schema 是否匹配".to_string()),
        "RESUME_MISMATCH" => Some("回调 event 或 correlationKey/requestId 不匹配，确认外部系统没有重复或串单回调".to_string()),
        "TRANSITION_ERROR" => Some("检查当前节点分支配置，确认 branchKey 能命中下游连线".to_string()),
        "SUB_WORKFLOW_FAILED" | "SUB_WORKFLOW_ERROR" => {
            Some("继续查看子工作流 timeline，确认失败节点与上下游入参".to_string())
        }
        "SUB_WORKFLOW_TERMINATED" => Some("确认子工作流是否被人工终止或被上游取消".to_string()),
        _ => None,
    }
}
