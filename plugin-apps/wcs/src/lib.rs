mod config;
mod controllers;
mod descriptors;
mod models;
mod router;
mod services;
mod views;

use crate::router::build_router;
use crate::services::AppState;
use axum::Router;

pub use config::{AppConfig, DEFAULT_CONNECT_WORKER_ID, DEFAULT_RUNNER_RESUME_SIGNAL, HEALTH_PLUGIN_ID};

pub fn build_app() -> Router {
    build_app_with_config(AppConfig::from_env())
}

pub fn build_app_with_config(config: AppConfig) -> Router {
    let state = AppState::new(config);
    build_router(state)
}

#[cfg(test)]
mod tests;
