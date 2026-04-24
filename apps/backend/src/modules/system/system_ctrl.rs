use axum::Json;
use axum::extract::State;
use tracing::debug;

use super::system_service::{self, HealthResponse, PluginAutoRegistrationConfig, UpdatePluginAutoRegistrationResponse};
use crate::modules::{ApiError, ApiState};

pub async fn health() -> Json<HealthResponse> {
    debug!("health check requested");
    Json(system_service::health())
}

pub async fn get_plugin_auto_registration(
    State(state): State<ApiState>,
) -> Result<Json<PluginAutoRegistrationConfig>, ApiError> {
    Ok(Json(system_service::get_plugin_auto_registration(&state).await?))
}

pub async fn update_plugin_auto_registration(
    State(state): State<ApiState>,
    Json(request): Json<PluginAutoRegistrationConfig>,
) -> Result<Json<UpdatePluginAutoRegistrationResponse>, ApiError> {
    Ok(Json(
        system_service::update_plugin_auto_registration(&state, request).await?,
    ))
}
