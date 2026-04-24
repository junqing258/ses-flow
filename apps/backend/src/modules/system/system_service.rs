use std::collections::HashSet;

use runner::services::{NodeDescriptor, normalize_base_url};
use serde::{Deserialize, Serialize};

use crate::modules::node_registry::register_http_plugin_base_urls;
use crate::modules::{ApiError, ApiState};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAutoRegistrationConfig {
    #[serde(default)]
    pub base_urls: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginAutoRegistrationResponse {
    pub base_urls: Vec<String>,
    pub descriptors: Vec<NodeDescriptor>,
}

pub fn health() -> HealthResponse {
    HealthResponse { status: "ok" }
}

pub async fn get_plugin_auto_registration(state: &ApiState) -> Result<PluginAutoRegistrationConfig, ApiError> {
    let base_urls = state
        .system_settings
        .load_plugin_auto_register_base_urls()
        .await
        .map_err(ApiError::ServiceUnavailable)?;

    Ok(PluginAutoRegistrationConfig {
        base_urls: normalize_base_urls(&base_urls),
    })
}

pub async fn update_plugin_auto_registration(
    state: &ApiState,
    request: PluginAutoRegistrationConfig,
) -> Result<UpdatePluginAutoRegistrationResponse, ApiError> {
    let next_base_urls = normalize_base_urls(&request.base_urls);
    let previous_base_urls = state
        .system_settings
        .load_plugin_auto_register_base_urls()
        .await
        .map_err(ApiError::ServiceUnavailable)?;

    let descriptors = register_http_plugin_base_urls(state, &next_base_urls).await?;

    let next_base_url_set = next_base_urls.iter().collect::<HashSet<_>>();
    let stale_base_urls = previous_base_urls
        .into_iter()
        .filter(|base_url| !next_base_url_set.contains(base_url))
        .collect::<Vec<_>>();

    if !stale_base_urls.is_empty() {
        state
            .app
            .unregister_node_descriptors_by_endpoints(&stale_base_urls)
            .map_err(ApiError::from)?;
    }

    state
        .system_settings
        .save_plugin_auto_register_base_urls(&next_base_urls)
        .await
        .map_err(ApiError::ServiceUnavailable)?;

    Ok(UpdatePluginAutoRegistrationResponse {
        base_urls: next_base_urls,
        descriptors,
    })
}

pub fn normalize_base_urls(base_urls: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();

    base_urls
        .iter()
        .flat_map(|value| value.split([',', '\n']))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| normalize_base_url(value.to_string()))
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
