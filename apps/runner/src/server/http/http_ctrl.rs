use axum::Router;
use axum::middleware;
use axum::response::Redirect;
use axum::routing::{get, post, put};

use super::http_service::{
    ApiState, RUNNER_API_BASE_PATH, RUNNER_VIEWS_BASE_PATH, build_cors_layer, build_views_service, log_http_requests,
};
use crate::server::{edit_session, run, system, workflow};

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/", get(redirect_to_views))
        .nest_service(RUNNER_VIEWS_BASE_PATH, build_views_service())
        .nest(RUNNER_API_BASE_PATH, build_api_router(state))
}

fn build_api_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(system::health))
        .route("/workflows/events", get(workflow::subscribe_workflows_events))
        .route(
            "/workflows",
            get(workflow::list_workflows).post(workflow::upload_workflow),
        )
        .route("/workflows/{workflow_id}", get(workflow::get_workflow))
        .route(
            "/workflows/{workflow_id}/events",
            get(workflow::subscribe_workflow_events),
        )
        .route("/workflows/{workflow_id}/runs", get(workflow::list_workflow_runs))
        .route("/workflows/{workflow_id}/run", post(run::execute_workflow))
        .route("/edit-sessions", post(edit_session::create_edit_session))
        .route("/edit-sessions/{session_id}", get(edit_session::get_edit_session))
        .route(
            "/edit-sessions/{session_id}/events",
            get(edit_session::subscribe_edit_session_events),
        )
        .route(
            "/edit-sessions/{session_id}/draft",
            put(edit_session::update_edit_session),
        )
        .route("/runs/{run_id}", get(run::get_run_summary))
        .route("/runs/{run_id}/events", get(run::subscribe_run_events))
        .route("/runs/{run_id}/resume", post(run::resume_workflow))
        .route("/runs/{run_id}/terminate", post(run::terminate_workflow))
        .layer(middleware::from_fn(log_http_requests))
        .layer(build_cors_layer())
        .with_state(state)
}

async fn redirect_to_views() -> Redirect {
    Redirect::permanent("/views/")
}
