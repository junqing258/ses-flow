use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

use crate::controllers::plugin;

pub fn build_app() -> Router {
    Router::new()
        .route("/descriptors", get(plugin::get_descriptors))
        .route("/descriptor", get(plugin::get_descriptor))
        .route("/health", get(plugin::get_health))
        .route("/execute", post(plugin::execute))
        .route("/cancel", post(plugin::cancel))
        .route("/resume", post(plugin::resume))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}
