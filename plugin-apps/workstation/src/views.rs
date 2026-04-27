use std::convert::Infallible;

use async_stream::stream;
use axum::Json;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Response, Sse};
use chrono::Utc;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::models::{
    BaseResult, ExecutionTask, PendingEvent, PluginLogRecord, PluginResponseEnvelope, TaskErrorPayload, TaskSnapshot,
    WaitSignal,
};

pub(crate) fn plugin_waiting_response(task: &ExecutionTask) -> Response {
    Json(PluginResponseEnvelope {
        status: "waiting".to_string(),
        output: json!({
            "executionId": task.execution_id,
            "workerId": task.target_worker_id,
            "taskId": task.task_id
        }),
        state_patch: json!({
            "workstation": {
                "executions": {
                    task.execution_id.clone(): {
                        "status": task.state.as_str(),
                        "workerId": task.target_worker_id,
                        "taskId": task.task_id
                    }
                }
            }
        }),
        wait_signal: Some(WaitSignal {
            signal_type: task.wait_signal_type.clone(),
            payload: json!({
                "executionId": task.execution_id,
                "requestId": task.request_id
            }),
        }),
        logs: vec![PluginLogRecord {
            level: "info".to_string(),
            message: "manual task dispatched to workstation bridge".to_string(),
            fields: json!({
                "executionId": task.execution_id,
                "workerId": task.target_worker_id,
                "pluginType": task.plugin_type
            }),
        }],
        error: None,
    })
    .into_response()
}

pub(crate) fn success_resume_event(task: &ExecutionTask, output: Value, state_patch: Value) -> Value {
    json!({
        "type": task.wait_signal_type,
        "payload": {
            "status": "success",
            "output": output,
            "statePatch": state_patch
        }
    })
}

pub(crate) fn failure_resume_event(task: &ExecutionTask, error: &TaskErrorPayload) -> Value {
    json!({
        "type": task.wait_signal_type,
        "payload": {
            "status": "failed",
            "error": {
                "code": error.code,
                "message": error.message,
                "retryable": false
            }
        }
    })
}

pub(crate) fn sync_snapshot_event(worker_id: &str, tasks: Vec<TaskSnapshot>) -> Event {
    Event::default().event("sync.snapshot").data(
        json!({
            "MessageType": "sync.snapshot",
            "WorkerId": worker_id,
            "Tasks": tasks
        })
        .to_string(),
    )
}

pub(crate) fn heartbeat_event(worker_id: &str) -> Event {
    Event::default().event("Heart_Beat").data(
        json!({
            "MessageType": "Heart_Beat",
            "WorkerId": worker_id,
            "RcsStatus": "connected",
            "StationList": [worker_id],
            "Ts": Utc::now().to_rfc3339()
        })
        .to_string(),
    )
}

pub(crate) fn sse_response(
    worker_id: String,
    heartbeat_interval_secs: u64,
    mut receiver: broadcast::Receiver<PendingEvent>,
    backlog: Vec<PendingEvent>,
    snapshots: Vec<TaskSnapshot>,
) -> Response {
    let stream = stream! {
        for event in backlog {
            yield Ok::<Event, Infallible>(event.to_sse_event());
        }
        yield Ok(sync_snapshot_event(&worker_id, snapshots));

        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(heartbeat_interval_secs));
        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(event) => yield Ok(event.to_sse_event()),
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat.tick() => {
                    yield Ok(heartbeat_event(&worker_id));
                }
            }
        }
    };

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(heartbeat_interval_secs)))
        .into_response()
}

pub(crate) fn plugin_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

pub(crate) fn base_result_ok(data: Value) -> Response {
    Json(BaseResult {
        code: 0,
        message: "Success".to_string(),
        data,
    })
    .into_response()
}

pub(crate) fn base_result_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(BaseResult {
            code: status.as_u16() as i32,
            message: message.to_string(),
            data: Value::Null,
        }),
    )
        .into_response()
}
