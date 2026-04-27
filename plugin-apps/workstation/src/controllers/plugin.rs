use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::descriptors::plugin_descriptors;
use crate::models::{CancelRequest, ExecuteRequest, HealthResponse, PluginDescriptor, ResumeRequest};
use crate::services::AppState;
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
