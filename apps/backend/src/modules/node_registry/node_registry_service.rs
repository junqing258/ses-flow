use axum::http::StatusCode;
use runner::services::{NodeDescriptor, RegisteredHttpPluginDescriptor};
use serde::{Deserialize, Serialize};

use crate::modules::{ApiError, ApiState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterHttpPluginRequest {
    pub base_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterHttpPluginResponse {
    pub descriptor: NodeDescriptor,
}

pub async fn register_http_plugin(
    state: &ApiState,
    request: RegisterHttpPluginRequest,
) -> Result<(StatusCode, RegisterHttpPluginResponse), ApiError> {
    let base_url = request.base_url.trim();
    if base_url.is_empty() {
        return Err(ApiError::BadRequest("plugin baseUrl is required".to_string()));
    }

    let descriptor = state
        .ai_gateway_client
        .get(format!("{}/descriptor", base_url.trim_end_matches('/')))
        .send()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(format!("failed to fetch plugin descriptor: {error}")))?;

    if !descriptor.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "plugin descriptor endpoint returned {}",
            descriptor.status()
        )));
    }

    let descriptor = descriptor
        .json::<NodeDescriptor>()
        .await
        .map_err(|error| ApiError::BadRequest(format!("failed to parse plugin descriptor: {error}")))?;
    let registered = RegisteredHttpPluginDescriptor::new(descriptor, base_url.to_string()).map_err(ApiError::Runner)?;
    let descriptor = state.app.register_node_descriptor(registered.descriptor)?;

    Ok((StatusCode::CREATED, RegisterHttpPluginResponse { descriptor }))
}

pub fn list_node_descriptors(state: &ApiState) -> Result<Vec<NodeDescriptor>, ApiError> {
    state.app.list_node_descriptors().map_err(ApiError::from)
}

pub fn get_node_descriptor_versions(state: &ApiState, descriptor_id: &str) -> Result<Vec<NodeDescriptor>, ApiError> {
    state
        .app
        .list_node_descriptor_versions(descriptor_id)
        .map_err(ApiError::from)
}
