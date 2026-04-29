use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::descriptors::plugin_descriptors;
use crate::models::{
    CancelRequest, ExecuteRequest, HealthResponse, PluginDescriptor, PluginResponseEnvelope, ResumeRequest,
};
use crate::services::{AppState, PendingRobotDeparture};
use crate::views::{plugin_error, plugin_waiting_response};

pub(crate) async fn get_descriptors() -> Json<Vec<PluginDescriptor>> {
    Json(plugin_descriptors())
}

pub(crate) async fn get_descriptor() -> Json<PluginDescriptor> {
    Json(
        plugin_descriptors()
            .into_iter()
            .next()
            .expect("workstation plugin should expose at least one descriptor"),
    )
}

pub(crate) async fn get_health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(state.health().await)
}

pub(crate) async fn execute(State(state): State<AppState>, Json(request): Json<ExecuteRequest>) -> Response {
    if request.runner_type == "plugin:get_task_info" {
        return match get_task_info_plugin_response(request) {
            Ok(response) => Json(response).into_response(),
            Err(message) => plugin_error(StatusCode::BAD_REQUEST, &message),
        };
    }

    if request.runner_type == "plugin:robot_departure" {
        if let Some(response) = robot_departure_plugin_response(&state, &request).await {
            return Json(response).into_response();
        }
    }

    match state.create_or_get_task(request).await {
        Ok(task) => plugin_waiting_response(&task),
        Err(message) => plugin_error(StatusCode::BAD_REQUEST, &message),
    }
}

pub(crate) async fn cancel(State(state): State<AppState>, Json(request): Json<CancelRequest>) -> Response {
    match state.cancel_task(request).await {
        Ok(()) => Json(json!({ "status": "ok" })).into_response(),
        Err(message) => plugin_error(StatusCode::NOT_FOUND, &message),
    }
}

pub(crate) async fn resume(State(state): State<AppState>, Json(request): Json<ResumeRequest>) -> Response {
    match state.resume_external(request).await {
        Ok(()) => Json(json!({ "status": "ok" })).into_response(),
        Err(message) => plugin_error(StatusCode::NOT_FOUND, &message),
    }
}

fn get_task_info_plugin_response(request: ExecuteRequest) -> Result<PluginResponseEnvelope, String> {
    let input = request.context.input;
    let station_id = value_string(&input, &["stationId", "workerId", "targetWorkerId"])
        .ok_or_else(|| "missing stationId in get_task_info input".to_string())?;
    let item_key = value_string(&input, &["sku"])
        .or_else(|| value_string(&input, &["itemId", "barcode"]))
        .ok_or_else(|| "missing sku/itemId/barcode in get_task_info input".to_string())?;
    let task_id = format!("TASK-{item_key}");
    let count = value_i64(&input, &["completed", "count"]).unwrap_or(0) + 1;
    let target_id = format!("C-{}", item_key.chars().take(3).collect::<String>());
    let request_id = value_string(&input, &["requestId"]).unwrap_or(request.context.request_id);
    let agv_id = value_string(&input, &["agvId"]).unwrap_or_else(|| "AGV-001".to_string());
    let order_id = format!("ORDER-{station_id}");
    let order_detail_id = format!("DETAIL-{item_key}");
    let output = json!({
        "taskId": task_id,
        "orderId": order_id,
        "orderDetailId": order_detail_id,
        "targetId": target_id,
        "count": count,
        "completed": count,
        "stationId": station_id,
        "agvId": agv_id,
        "requestId": request_id
    });

    Ok(PluginResponseEnvelope {
        status: "success".to_string(),
        output: output.clone(),
        state_patch: json!({
            "taskId": output["taskId"],
            "orderId": output["orderId"],
            "orderDetailId": output["orderDetailId"],
            "targetId": output["targetId"],
            "count": output["count"],
            "stationId": output["stationId"],
            "agvId": output["agvId"],
            "requestId": output["requestId"],
            "workstation": {
                "lastTaskInfo": output
            }
        }),
        wait_signal: None,
        logs: Vec::new(),
        error: None,
    })
}

async fn robot_departure_plugin_response(state: &AppState, request: &ExecuteRequest) -> Option<PluginResponseEnvelope> {
    let input = &request.context.input;
    let station_id = value_string(input, &["stationId", "workerId", "targetWorkerId"])?;
    let task_id = value_string(input, &["taskId"])?;
    let departure = state.take_pending_robot_departure(&station_id, &task_id).await?;
    Some(robot_departure_success_response(departure))
}

fn robot_departure_success_response(departure: PendingRobotDeparture) -> PluginResponseEnvelope {
    let output = json!({
        "taskId": departure.task_id,
        "agvId": departure.agv_id,
        "completed": departure.completed,
        "requestId": departure.request_id,
        "stationId": departure.station_id,
        "result": 0
    });

    PluginResponseEnvelope {
        status: "success".to_string(),
        output: output.clone(),
        state_patch: json!({
            "workstation": {
                "lastRobotDeparture": output
            }
        }),
        wait_signal: None,
        logs: Vec::new(),
        error: None,
    }
}

fn value_string(value: &Value, candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|key| {
        value.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    })
}

fn value_i64(value: &Value, candidates: &[&str]) -> Option<i64> {
    candidates.iter().find_map(|key| {
        value.get(key).and_then(Value::as_i64).or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get(key))
                .and_then(Value::as_i64)
        })
    })
}
