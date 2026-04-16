use axum::Json;
use tracing::debug;

use super::system_service::{self, HealthResponse};

pub async fn health() -> Json<HealthResponse> {
    debug!("health check requested");
    Json(system_service::health())
}
