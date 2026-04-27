use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use tracing::{debug, info};

use crate::controllers::{plugin, station};
use crate::services::AppState;

pub(crate) fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/descriptors", get(plugin::get_descriptors))
        .route("/descriptor", get(plugin::get_descriptor))
        .route("/health", get(plugin::get_health))
        .route("/execute", post(plugin::execute))
        .route("/cancel", post(plugin::cancel))
        .route("/resume", post(plugin::resume))
        .route("/station/operation/login", post(station::login))
        .route("/station/operation/connect", post(station::connect))
        .route(
            "/station/operation/simulate/agvArrived",
            post(station::simulate_agv_arrived),
        )
        .route(
            "/station/operation/simulateAgvArrived",
            post(station::simulate_agv_arrived),
        )
        .route("/station/operation/synchronize", post(station::synchronize))
        .route("/station/operation/verifyNotify", post(station::verify_notify))
        .route("/station/operation/scanBarcode", post(station::scan_barcode))
        .route("/station/operation/getTaskInfo", post(station::get_task_info))
        .route("/station/operation/robotDeparture", post(station::robot_departure))
        .route("/station/operation/driveOutRobot", post(station::drive_out_robot))
        .route(
            "/station/operation/noBarcodeForceDepart",
            post(station::no_barcode_force_depart),
        )
        .route("/station/operation/tasks/{execution_id}/fail", post(station::fail_task))
        .layer(middleware::from_fn(log_http_requests))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state)
}

async fn log_http_requests(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .or_else(|| request.headers().get("requestId").and_then(|value| value.to_str().ok()))
        .unwrap_or("")
        .to_string();
    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or(uri.path())
        .to_string();
    let start = Instant::now();

    debug!(method = %method, uri = %uri, "started request");

    let response = next.run(request).await;

    info!(
        method = %method,
        matched_path = %matched_path,
        uri = %uri,
        request_id = %request_id,
        status = response.status().as_u16(),
        latency_ms = start.elapsed().as_millis(),
        "finished request",
    );

    response
}
