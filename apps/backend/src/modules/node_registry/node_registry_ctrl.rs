use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use super::node_registry_service::{self, RegisterHttpPluginRequest, RegisterHttpPluginResponse};
use crate::modules::{ApiError, ApiState};

pub async fn register_http_plugin(
    State(state): State<ApiState>,
    Json(request): Json<RegisterHttpPluginRequest>,
) -> Result<(StatusCode, Json<RegisterHttpPluginResponse>), ApiError> {
    let (status, response) = node_registry_service::register_http_plugin(&state, request).await?;
    Ok((status, Json(response)))
}

pub async fn list_node_descriptors(
    State(state): State<ApiState>,
) -> Result<Json<Vec<runner::services::NodeDescriptor>>, ApiError> {
    Ok(Json(node_registry_service::list_node_descriptors(&state)?))
}

pub async fn get_node_descriptor_versions(
    State(state): State<ApiState>,
    Path(descriptor_id): Path<String>,
) -> Result<Json<Vec<runner::services::NodeDescriptor>>, ApiError> {
    Ok(Json(node_registry_service::get_node_descriptor_versions(
        &state,
        &descriptor_id,
    )?))
}
