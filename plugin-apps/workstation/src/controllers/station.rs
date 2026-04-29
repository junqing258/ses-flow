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
use crate::services::{AppState, station_id_from_connect};
use crate::views::{base_result_error, base_result_ok, sse_response};

pub(crate) async fn login(State(state): State<AppState>, Json(request): Json<LoginRequest>) -> Response {
    let token = match state.login(&request).await {
        Ok(token) => token,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
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
    let station_id = station_id_from_connect(&request);
    let heartbeat_interval_secs = state.heartbeat_interval_secs();
    let (receiver, backlog, snapshots) = state.connect_context(&station_id, query.since).await;
    sse_response(station_id, heartbeat_interval_secs, receiver, backlog, snapshots)
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
        "StationId": simulation.event.station_id,
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
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    match state.verify_notify(&station_id, request).await {
        Ok(()) => base_result_ok(Value::Null),
        Err(message) => base_result_error(StatusCode::NOT_FOUND, &message),
    }
}

pub(crate) async fn offline(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    base_result_ok(json!({
        "StationId": station_id,
        "Status": 0,
        "Online": false
    }))
}

pub(crate) async fn online(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    base_result_ok(json!({
        "StationId": station_id,
        "Status": 1,
        "Online": true
    }))
}

pub(crate) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    base_result_ok(json!({
        "StationId": station_id,
        "LoggedOut": true
    }))
}

pub(crate) async fn scan_barcode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BarcodeRequest>,
) -> Response {
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = state.current_task_for_worker(&station_id).await;
    let barcode = request.barcode;
    let request_id = request
        .request_id
        .or_else(|| task.as_ref().map(|item| item.request_id.clone()));
    let sku = task
        .as_ref()
        .map(|item| item.task_id.clone())
        .unwrap_or_else(|| format!("SKU-{}", barcode));
    let resumed_run_ids = state
        .resume_scan_barcode_waits(&station_id, &barcode, &sku, request_id.as_deref())
        .await;
    base_result_ok(json!({
        "Items": [
            {
                "Sku": sku,
                "ItemId": barcode,
                "BarcodeName": format!("商品-{}", barcode),
                "BarCode": barcode
            }
        ],
        "Barcode": barcode,
        "WorkerId": station_id,
        "TaskId": task.as_ref().map(|item| item.task_id.clone()),
        "ExecutionId": task.as_ref().map(|item| item.execution_id.clone()),
        "ResumedRunIds": resumed_run_ids
    }))
}

pub(crate) async fn get_task_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TaskInfoRequest>,
) -> Response {
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = state.current_task_for_worker(&station_id).await;
    let task_id = task.as_ref().map(|task| task.task_id.clone()).unwrap_or_else(|| {
        let task_key = request.sku.as_deref().or(request.barcode.as_deref()).unwrap_or("MOCK");
        format!("TASK-{}", task_key)
    });
    let order_id = task
        .as_ref()
        .map(|task| task.run_id.clone())
        .unwrap_or_else(|| format!("ORDER-{}", station_id));
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
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
        Err(message) => return base_result_error(StatusCode::UNAUTHORIZED, &message),
    };
    let task = match request.task_id.as_deref() {
        Some(task_id) => state.robot_departure_task_for_worker(&station_id, task_id).await,
        None => state.current_robot_departure_task_for_worker(&station_id).await,
    };
    let task_id = request
        .task_id
        .or_else(|| task.as_ref().map(|task| task.task_id.clone()));
    let task_id = match task_id {
        Some(task_id) => Some(task_id),
        None => {
            state
                .robot_departure_wait_task_id_for_worker(&station_id, &request.request_id)
                .await
        }
    };
    let Some(task_id) = task_id else {
        return base_result_error(
            StatusCode::BAD_REQUEST,
            "missing taskId and no active robot departure task for station",
        );
    };
    let agv_event_payload = json!({
        "AgvId": request.agv_id,
        "StationId": station_id,
        "TaskId": &task_id,
        "RequestId": request.request_id
    });
    let state_patch = json!({
        "workstation": {
            "lastRobotDeparture": {
                "taskId": &task_id,
                "agvId": request.agv_id,
                "completed": request.completed,
                "requestId": request.request_id
            }
        }
    });
    let output = json!({
        "taskId": &task_id,
        "agvId": request.agv_id,
        "completed": request.completed,
        "requestId": request.request_id
    });
    let resumed_run_ids = if let Some(task) = task {
        match state
            .complete_task_by_execution_id_with_success(
                &task.execution_id,
                &station_id,
                request.request_id,
                output.clone(),
                state_patch.clone(),
                Some(agv_event_payload),
            )
            .await
        {
            Ok(()) => Vec::new(),
            Err(message) => return base_result_error(StatusCode::BAD_REQUEST, &message),
        }
    } else {
        state
            .queue_pending_event(&station_id, None, "AGV_DEPART", agv_event_payload)
            .await;
        info!(
            station_id = %station_id,
            task_id = %task_id,
            agv_id = %request.agv_id,
            "simulated robot departure without active runner task"
        );
        let resumed_run_ids = state
            .resume_robot_departure_waits(
                &station_id,
                &task_id,
                &request.agv_id,
                request.completed,
                &request.request_id,
            )
            .await;
        if resumed_run_ids.is_empty() {
            state
                .record_pending_robot_departure(
                    &station_id,
                    &task_id,
                    &request.agv_id,
                    request.completed,
                    &request.request_id,
                )
                .await;
        }
        resumed_run_ids
    };
    base_result_ok(json!({
        "AgvId": request.agv_id,
        "TaskId": &task_id,
        "ResumedRunIds": resumed_run_ids
    }))
}

pub(crate) async fn drive_out_robot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DriveOutRobotRequest>,
) -> Response {
    match state.authenticated_station_id(&headers).await {
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
    let station_id = match state.authenticated_station_id(&headers).await {
        Ok(station_id) => station_id,
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
        .complete_task_with_success(&station_id, request.request_id, output, state_patch, None)
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
    if let Err(message) = state.authenticated_station_id(&headers).await {
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
