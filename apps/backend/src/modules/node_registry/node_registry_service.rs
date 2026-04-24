use axum::http::StatusCode;
use runner::services::{NodeDescriptor, RegisteredHttpPluginDescriptor};
use serde::{Deserialize, Serialize};

use crate::modules::{ApiError, ApiState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginHealthResponse {
    #[serde(default)]
    plugin_id: Option<String>,
    #[serde(default)]
    plugin_name: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    app_name: Option<String>,
}

#[derive(Debug, Clone)]
struct PluginApplicationMetadata {
    app_id: Option<String>,
    app_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterHttpPluginRequest {
    pub base_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterHttpPluginResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<NodeDescriptor>,
    pub descriptors: Vec<NodeDescriptor>,
}

pub async fn register_http_plugin(
    state: &ApiState,
    request: RegisterHttpPluginRequest,
) -> Result<(StatusCode, RegisterHttpPluginResponse), ApiError> {
    let descriptors = register_http_plugin_base_url(state, &request.base_url).await?;
    let descriptor = descriptors.first().cloned();

    Ok((
        StatusCode::CREATED,
        RegisterHttpPluginResponse {
            descriptor,
            descriptors,
        },
    ))
}

pub async fn register_http_plugin_base_url(state: &ApiState, base_url: &str) -> Result<Vec<NodeDescriptor>, ApiError> {
    let base_url = base_url.trim();
    if base_url.is_empty() {
        return Err(ApiError::BadRequest("plugin baseUrl is required".to_string()));
    }

    let mut descriptors = fetch_http_plugin_descriptors(state, base_url).await?;
    let app_metadata = fetch_http_plugin_application_metadata(state, base_url).await;

    descriptors.iter_mut().for_each(|descriptor| {
        if let Some(metadata) = app_metadata.as_ref() {
            descriptor.plugin_app_id = metadata.app_id.clone();
            descriptor.plugin_app_name = metadata.app_name.clone();
        }
    });

    let mut registered_descriptors = Vec::with_capacity(descriptors.len());

    for descriptor in descriptors {
        let registered =
            RegisteredHttpPluginDescriptor::new(descriptor, base_url.to_string()).map_err(ApiError::Runner)?;
        registered_descriptors.push(
            state
                .app
                .register_node_descriptor(registered.descriptor)
                .map_err(ApiError::from)?,
        );
    }

    Ok(registered_descriptors)
}

async fn fetch_http_plugin_descriptors(state: &ApiState, base_url: &str) -> Result<Vec<NodeDescriptor>, ApiError> {
    let descriptors_url = format!("{}/descriptors", base_url.trim_end_matches('/'));
    let descriptors_response = state
        .ai_gateway_client
        .get(descriptors_url)
        .send()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(format!("failed to fetch plugin descriptors: {error}")))?;

    if descriptors_response.status().is_success() {
        let response_body = descriptors_response
            .text()
            .await
            .map_err(|error| ApiError::BadRequest(format!("failed to read plugin descriptors response: {error}")))?;
        let descriptors = parse_plugin_descriptors_response(&response_body)?;

        if descriptors.is_empty() {
            return Err(ApiError::BadRequest(
                "plugin descriptors endpoint returned an empty list".to_string(),
            ));
        }

        return Ok(descriptors);
    }

    if descriptors_response.status() != StatusCode::NOT_FOUND {
        return Err(ApiError::BadRequest(format!(
            "plugin descriptors endpoint returned {}",
            descriptors_response.status()
        )));
    }

    let descriptor_response = state
        .ai_gateway_client
        .get(format!("{}/descriptor", base_url.trim_end_matches('/')))
        .send()
        .await
        .map_err(|error| ApiError::ServiceUnavailable(format!("failed to fetch plugin descriptor: {error}")))?;

    if !descriptor_response.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "plugin descriptor endpoint returned {}",
            descriptor_response.status()
        )));
    }

    let descriptor = descriptor_response
        .json::<NodeDescriptor>()
        .await
        .map_err(|error| ApiError::BadRequest(format!("failed to parse plugin descriptor: {error}")))?;
    Ok(vec![descriptor])
}

fn parse_plugin_descriptors_response(response_body: &str) -> Result<Vec<NodeDescriptor>, ApiError> {
    if let Ok(descriptors) = serde_json::from_str::<Vec<NodeDescriptor>>(response_body) {
        return Ok(descriptors);
    }

    if let Ok(descriptor) = serde_json::from_str::<NodeDescriptor>(response_body) {
        return Ok(vec![descriptor]);
    }

    Err(ApiError::BadRequest(
        "failed to parse plugin descriptors: expected a descriptor object or array".to_string(),
    ))
}

async fn fetch_http_plugin_application_metadata(state: &ApiState, base_url: &str) -> Option<PluginApplicationMetadata> {
    let health_url = format!("{}/health", base_url.trim_end_matches('/'));
    let response = state.ai_gateway_client.get(health_url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let payload = response.json::<PluginHealthResponse>().await.ok()?;
    let app_id = payload.app_id.or(payload.plugin_id);
    let app_name = payload.app_name.or(payload.plugin_name).or(payload.display_name);

    if app_id.is_none() && app_name.is_none() {
        return None;
    }

    Some(PluginApplicationMetadata { app_id, app_name })
}

pub async fn register_http_plugin_base_urls(
    state: &ApiState,
    base_urls: &[String],
) -> Result<Vec<NodeDescriptor>, ApiError> {
    let mut descriptors = Vec::new();
    for base_url in base_urls {
        descriptors.extend(register_http_plugin_base_url(state, base_url).await?);
    }
    Ok(descriptors)
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
