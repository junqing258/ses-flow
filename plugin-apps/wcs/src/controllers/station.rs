use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use tracing::info;

use crate::models::{
    BarcodeRequest, BaseResult, ConnectQuery, ConnectRequest, DriveOutRobotRequest, FailTaskRequest, LoginData,
    LoginRequest, NoBarcodeForceDepartRequest, RobotDepartureRequest, TaskInfoRequest, TaskInfoResponseData,
    VerifyNotifyRequest,
};
use crate::services::{AppState, worker_id_from_connect};
use crate::views::{base_result_error, base_result_ok, sse_response};

pub(crate) async fn login(State(state): State<AppState>, Json(request): Json<LoginRequest>) -> Response {
    let token = state.login(&request.station_id).await;
    Json(BaseResult {
        code: 0,
        message: "Success".to_string(),
        data: LoginData {
            authorization: format!("Bearer {token}"),
        },
    })
    .into_response()
}

pub(crate) async fn synchronize() -> Response {
    base_result_ok(Value::Null)
}

pub(crate) async fn connect(
    State(state): State<AppState>,
    Query(query): Query<ConnectQuery>,
    Json(request): Json<ConnectRequest>,
) -> Response {
    let worker_id = worker_id_from_connect(&request);
    let heartbeat_interval_secs = state.heartbeat_interval_secs();
    let (receiver, backlog, snapshots) = state.connect_context(&worker_id, query.since).await;
    sse_response(worker_id, heartbeat_interval_secs, receiver, backlog, snapshots)
}

pub(crate) async fn verify_notify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<VerifyNotifyRequest>,
) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    match state.verify_notify(&worker_id, request).await {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::NOT_FOUND, &message),
    }
}

pub(crate) async fn scan_barcode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BarcodeRequest>,
) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = state.current_task_for_worker(&worker_id).await;
    base_result_ok(json!({
        "Barcode": request.barcode,
        "WorkerId": worker_id,
        "TaskId": task.as_ref().map(|item| item.task_id.clone()),
        "ExecutionId": task.as_ref().map(|item| item.execution_id.clone())
    }))
}

pub(crate) async fn get_task_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TaskInfoRequest>,
) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = match state.current_task_for_worker(&worker_id).await {
        Some(task) => task,
        None => return base_result_error(StatusCode::NOT_FOUND, "no active task for worker"),
    };
    let data = TaskInfoResponseData {
        task_id: task.task_id,
        chute_id: request
            .sku
            .or(request.barcode)
            .map(|value| format!("C-{}", value.chars().take(3).collect::<String>()))
            .unwrap_or_else(|| "C01".to_string()),
        wave_id: request.wave_type.unwrap_or_else(|| "WAVE-DEMO".to_string()),
        order_id: task.run_id,
        count: request.completed.unwrap_or(0) + 1,
    };
    Json(BaseResult {
        code: 0,
        message: "Success".to_string(),
        data,
    })
    .into_response()
}

pub(crate) async fn robot_departure(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RobotDepartureRequest>,
) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let agv_event_payload = json!({
        "AgvId": request.agv_id,
        "StationId": worker_id,
        "TaskId": request.task_id,
        "RequestId": request.request_id
    });
    let state_patch = json!({
        "wcs": {
            "lastRobotDeparture": {
                "taskId": request.task_id,
                "agvId": request.agv_id,
                "completed": request.completed,
                "requestId": request.request_id
            }
        }
    });
    let output = json!({
        "taskId": request.task_id,
        "agvId": request.agv_id,
        "completed": request.completed,
        "requestId": request.request_id
    });
    match state
        .complete_task_with_success(
            &worker_id,
            request.request_id,
            output,
            state_patch,
            Some(agv_event_payload),
        )
        .await
    {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
    }
}

pub(crate) async fn drive_out_robot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DriveOutRobotRequest>,
) -> Response {
    match state.authenticated_worker_id(&headers).await {
        Ok(_) => base_result_ok(json!({
            "AgvId": request.agv_id,
            "StationId": request.station_id,
            "Forced": true
        })),
        Err(message) => base_result_error(StatusCode::UNAUTHORIZED, &message),
    }
}

pub(crate) async fn no_barcode_force_depart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NoBarcodeForceDepartRequest>,
) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let output = json!({
        "taskId": request.task_id,
        "agvId": request.agv_id,
        "requestId": request.request_id,
        "barcodeMissing": true
    });
    let state_patch = json!({
        "wcs": {
            "lastRobotDeparture": {
                "taskId": request.task_id,
                "agvId": request.agv_id,
                "requestId": request.request_id,
                "barcodeMissing": true
            }
        }
    });
    match state
        .complete_task_with_success(&worker_id, request.request_id, output, state_patch, None)
        .await
    {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
    }
}

pub(crate) async fn fail_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<String>,
    Json(request): Json<FailTaskRequest>,
) -> Response {
    if let Err(message) = state.authenticated_worker_id(&headers).await {
        return base_result_error(StatusCode::UNAUTHORIZED, &message);
    }
    match state.fail_task(&execution_id, request.error).await {
        Ok(()) => {
            info!(execution_id = %execution_id, request_id = %request.request_id, "worker reported WCS task failure");
            base_result_ok(Value::Null)
        }
        Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
    }
}
