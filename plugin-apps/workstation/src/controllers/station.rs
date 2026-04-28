use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use tracing::info;

use crate::models::{
    BarcodeRequest, BaseResult, ConnectQuery, ConnectRequest, DriveOutRobotRequest, FailTaskRequest, LoginData,
    LoginRequest, NoBarcodeForceDepartRequest, RobotDepartureRequest, SimulateAgvArrivedRequest, StationStatusSyncData,
    StationStatusSyncRequest, TaskInfoRequest, TaskInfoResponseData, VerifyNotifyRequest,
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

pub(crate) async fn synchronize(Json(request): Json<StationStatusSyncRequest>) -> Response {
    base_result_ok(json!(StationStatusSyncData {
        station_id: request.station_id,
        status: request.status,
        platform_id: request.platform_id,
    }))
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

pub(crate) async fn simulate_agv_arrived(
    State(state): State<AppState>,
    Json(request): Json<SimulateAgvArrivedRequest>,
) -> Response {
    let simulation = state
        .simulate_agv_arrived(&request.station_id, &request.agv_id, request.request_id)
        .await;
    base_result_ok(json!({
        "EventId": simulation.event.event_id,
        "RequestId": simulation.event.request_id,
        "StationId": simulation.event.worker_id,
        "AgvId": request.agv_id,
        "MessageType": simulation.event.message_type,
        "ResumedRunIds": simulation.resumed_run_ids
    }))
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

pub(crate) async fn offline(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    base_result_ok(json!({
        "StationId": worker_id,
        "Status": 0,
        "Online": false
    }))
}

pub(crate) async fn online(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    base_result_ok(json!({
        "StationId": worker_id,
        "Status": 1,
        "Online": true
    }))
}

pub(crate) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let worker_id = match state.authenticated_worker_id(&headers).await {
        Ok(worker_id) => worker_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    base_result_ok(json!({
        "StationId": worker_id,
        "LoggedOut": true
    }))
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
        "Items": [
            {
                "Sku": task
                    .as_ref()
                    .map(|item| item.task_id.clone())
                    .unwrap_or_else(|| format!("SKU-{}", request.barcode)),
                "BarcodeName": format!("商品-{}", request.barcode),
                "BarCode": request.barcode
            }
        ],
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
    let task = state.current_task_for_worker(&worker_id).await;
    let task_id = task.as_ref().map(|task| task.task_id.clone()).unwrap_or_else(|| {
        let task_key = request.sku.as_deref().or(request.barcode.as_deref()).unwrap_or("MOCK");
        format!("TASK-{}", task_key)
    });
    let order_id = task
        .as_ref()
        .map(|task| task.run_id.clone())
        .unwrap_or_else(|| format!("ORDER-{}", worker_id));
    let data = TaskInfoResponseData {
        task_id,
        chute_id: request
            .sku
            .or(request.barcode)
            .map(|value| format!("C-{}", value.chars().take(3).collect::<String>()))
            .unwrap_or_else(|| "C01".to_string()),
        wave_id: request.wave_type.unwrap_or_else(|| "WAVE-DEMO".to_string()),
        order_id,
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
        "workstation": {
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
    if state.current_task_for_worker(&worker_id).await.is_some() {
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
            Ok(()) => base_result_ok(json!({
                "AgvId": request.agv_id,
                "TaskId": request.task_id
            })),
            Err(message) => base_result_error(StatusCode::BAD_REQUEST, &message),
        }
    } else {
        state
            .queue_pending_event(&worker_id, None, "AGV_DEPART", agv_event_payload)
            .await;
        info!(
            worker_id = %worker_id,
            task_id = %request.task_id,
            agv_id = %request.agv_id,
            "simulated robot departure without active runner task"
        );
        base_result_ok(json!({
            "AgvId": request.agv_id,
            "TaskId": request.task_id
        }))
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
        "workstation": {
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
