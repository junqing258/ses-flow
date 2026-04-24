use axum::Json;
use axum::http::StatusCode;
use axum::response::Response;

use crate::models::{CancelRequest, ExecuteRequest, HealthResponse, PluginDescriptor, ResumeRequest};
use crate::services;
use crate::views::json_response;

pub(crate) async fn get_descriptors() -> Json<Vec<PluginDescriptor>> {
    Json(services::plugin_descriptors())
}

pub(crate) async fn get_descriptor() -> Json<PluginDescriptor> {
    Json(services::plugin_descriptor())
}

pub(crate) async fn get_health() -> Json<HealthResponse> {
    Json(services::health_response())
}

pub(crate) async fn execute(Json(request): Json<ExecuteRequest>) -> Response {
    let (payload, trace_id) = services::execute_response(request);
    json_response(StatusCode::OK, &payload, trace_id.as_deref())
}

pub(crate) async fn cancel(Json(request): Json<CancelRequest>) -> Response {
    let payload = services::cancel_response(&request.node_id);
    json_response(StatusCode::NOT_IMPLEMENTED, &payload, None)
}

pub(crate) async fn resume(Json(request): Json<ResumeRequest>) -> Response {
    let payload = services::resume_response(&request.node_id);
    json_response(StatusCode::NOT_IMPLEMENTED, &payload, None)
}
