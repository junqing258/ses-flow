use std::time::Instant;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::extract::MatchedPath;
use axum::http::{HeaderMap, Request};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use opentelemetry::global;
use opentelemetry::propagation::Extractor;
use tracing::{Instrument, debug, field, info, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
        .route("/station/operation/offline", post(station::offline))
        .route("/station/operation/online", post(station::online))
        .route("/station/operation/logout", post(station::logout))
        .route(
            "/station/operation/simulate/agvArrived",
            post(station::simulate_agv_arrived),
        )
        .route("/station/operation/synchronize", post(station::synchronize))
        .route("/station/operation/verifyNotify", post(station::verify_notify))
        .route("/station/operation/scanBarcode", post(station::scan_barcode))
        .route("/station/operation/getTaskInfo", post(station::get_task_info))
        .route("/station/operation/robotDeparture", post(station::robot_departure))
        .route("/station/operation/driveOutRobot", post(station::drive_out_robot))
        .route("/station/operation/driverEmptyRobot", post(station::drive_out_robot))
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
    let route_name = format!("{method} {matched_path}");
    let request_span = info_span!(
        "http.request",
        "otel.name" = %route_name,
        "otel.kind" = "server",
        "otel.status_code" = field::Empty,
        "otel.status_description" = field::Empty,
        "http.request.method" = %method,
        "http.route" = %matched_path,
        "url.path" = %uri.path(),
        "url.query" = uri.query().unwrap_or(""),
        "http.response.status_code" = field::Empty,
        "request.id" = %request_id,
        "latency_ms" = field::Empty,
    );
    let parent_context =
        global::get_text_map_propagator(|propagator| propagator.extract(&HeaderExtractor(request.headers())));
    let _ = request_span.set_parent(parent_context);

    async move {
        let start = Instant::now();

        debug!(method = %method, uri = %uri, "started request");

        let response = next.run(request).await;

        let status = response.status();
        let latency_ms = start.elapsed().as_millis();
        let current_span = tracing::Span::current();
        current_span.record("http.response.status_code", status.as_u16());
        current_span.record("latency_ms", latency_ms as u64);
        if status.is_server_error() {
            current_span.record("otel.status_code", "error");
            current_span.record("otel.status_description", format!("HTTP {}", status.as_u16()));
        }

        if status.is_client_error() {
            warn!(
                method = %method,
                matched_path = %matched_path,
                uri = %uri,
                request_id = %request_id,
                status = status.as_u16(),
                latency_ms,
                "finished request",
            );
        } else {
            info!(
                method = %method,
                matched_path = %matched_path,
                uri = %uri,
                request_id = %request_id,
                status = status.as_u16(),
                latency_ms,
                "finished request",
            );
        }

        response
    }
    .instrument(request_span)
    .await
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|name| name.as_str()).collect()
    }
}
