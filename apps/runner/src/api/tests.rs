use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::time::sleep;
use tower::ServiceExt;

use crate::api::{ApiState, build_router};
use crate::server::WorkflowServer;

fn build_app() -> axum::Router {
    build_router(ApiState {
        server: Arc::new(WorkflowServer::new()),
    })
}

#[tokio::test]
async fn uploads_workflow_and_executes_run_to_completion() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "api-server-flow",
            "name": "API Server Flow",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": {
            "type": "object"
        },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "end_1" }
        ],
        "policies": {}
    });

    let upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/workflows")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "workspaceId": "ws-api",
                        "workspaceName": "Warehouse API",
                        "workflow": workflow
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(upload_response.status(), StatusCode::OK);
    let upload_body = upload_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let upload_payload: Value =
        serde_json::from_slice(&upload_body).expect("response body should be valid json");
    let workflow_id = upload_payload["workflowId"]
        .as_str()
        .expect("workflow id should be present")
        .to_string();

    let execute_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/workflows/{workflow_id}/runs"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "trigger": {
                            "body": {
                                "orderNo": "SO-API-1"
                            }
                        }
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(execute_response.status(), StatusCode::ACCEPTED);
    let execute_body = execute_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let execute_payload: Value =
        serde_json::from_slice(&execute_body).expect("response body should be valid json");
    let run_id = execute_payload["runId"]
        .as_str()
        .expect("run id should be present")
        .to_string();

    let summary = wait_for_terminal_status(app, &run_id).await;
    assert_eq!(summary["status"], json!("completed"));
    assert_eq!(summary["workflowKey"], json!("api-server-flow"));
}

#[tokio::test]
async fn streams_waiting_run_summary_over_sse() {
    let app = build_app();
    let workflow: Value = serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
        .expect("example workflow should deserialize");

    let upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/workflows")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "workspaceId": "ws-sse",
                        "workflow": workflow
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    let upload_body = upload_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let upload_payload: Value =
        serde_json::from_slice(&upload_body).expect("response body should be valid json");
    let workflow_id = upload_payload["workflowId"]
        .as_str()
        .expect("workflow id should be present")
        .to_string();

    let execute_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/workflows/{workflow_id}/runs"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "trigger": {
                            "headers": {
                                "requestId": "req-api-sse-1"
                            },
                            "body": {
                                "orderNo": "SO-API-SSE-1",
                                "bizType": "auto_sort"
                            }
                        }
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    let execute_body = execute_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let execute_payload: Value =
        serde_json::from_slice(&execute_body).expect("response body should be valid json");
    let run_id = execute_payload["runId"]
        .as_str()
        .expect("run id should be present")
        .to_string();

    let waiting_summary = wait_for_status(app.clone(), &run_id, "waiting").await;
    assert_eq!(waiting_summary["status"], json!("waiting"));

    let sse_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/runs/{run_id}/events"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(sse_response.status(), StatusCode::OK);
    let mut body = sse_response.into_body();
    let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
        .await
        .expect("sse should produce a frame")
        .expect("body frame future should succeed")
        .expect("sse body should not end immediately");
    let chunk = frame.into_data().expect("frame should contain data");
    let text = String::from_utf8(chunk.to_vec()).expect("frame should be valid utf8");
    assert!(text.contains("event: summary"));
    assert!(text.contains(&format!("\"runId\":\"{run_id}\"")));
    assert!(text.contains("\"status\":\"waiting\""));
}

async fn wait_for_terminal_status(app: axum::Router, run_id: &str) -> Value {
    for _ in 0..40 {
        let summary = get_summary(app.clone(), run_id).await;
        if summary["status"] != json!("running") {
            return summary;
        }
        sleep(Duration::from_millis(25)).await;
    }

    panic!("run {run_id} did not reach a terminal status in time");
}

async fn wait_for_status(app: axum::Router, run_id: &str, expected_status: &str) -> Value {
    for _ in 0..40 {
        let summary = get_summary(app.clone(), run_id).await;
        if summary["status"] == json!(expected_status) {
            return summary;
        }
        sleep(Duration::from_millis(25)).await;
    }

    panic!("run {run_id} did not reach status {expected_status} in time");
}

async fn get_summary(app: axum::Router, run_id: &str) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/runs/{run_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    serde_json::from_slice(&body).expect("response body should be valid json")
}
