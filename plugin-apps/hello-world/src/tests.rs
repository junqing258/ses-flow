use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::models::{ExecuteResponse, PluginDescriptor};
use crate::router::build_app;
use crate::{FORMAL_PLUGIN_ID, FORMAL_PLUGIN_RUNNER_TYPE, PLUGIN_ID, PLUGIN_RUNNER_TYPE};

#[tokio::test]
async fn descriptor_route_returns_plugin_metadata() {
    let response = build_app()
        .oneshot(Request::builder().uri("/descriptor").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let descriptor: PluginDescriptor = serde_json::from_slice(&body).unwrap();
    assert_eq!(descriptor.id, PLUGIN_ID);
    assert_eq!(descriptor.runner_type, PLUGIN_RUNNER_TYPE);
    assert_eq!(descriptor.transport, "http");
    assert_eq!(descriptor.color.as_deref(), Some("#0EA5E9"));
    assert_eq!(descriptor.icon.as_deref(), Some("sparkles"));
}

#[tokio::test]
async fn descriptors_route_returns_multiple_plugin_descriptors() {
    let response = build_app()
        .oneshot(Request::builder().uri("/descriptors").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let descriptors: Vec<PluginDescriptor> = serde_json::from_slice(&body).unwrap();
    assert_eq!(descriptors.len(), 2);
    assert_eq!(descriptors[0].runner_type, PLUGIN_RUNNER_TYPE);
    assert_eq!(descriptors[1].runner_type, FORMAL_PLUGIN_RUNNER_TYPE);
    assert_eq!(descriptors[1].color.as_deref(), Some("#7C3AED"));
    assert_eq!(descriptors[1].icon.as_deref(), Some("badge-check"));
}

#[tokio::test]
async fn execute_route_returns_success_payload_and_trace_header() {
    let payload = json!({
        "pluginId": PLUGIN_ID,
        "runnerType": PLUGIN_RUNNER_TYPE,
        "nodeId": "node-hello-1",
        "config": {
            "prefix": "Hi"
        },
        "context": {
            "runId": "run-1",
            "requestId": "req-1",
            "traceId": "trace-1",
            "workflowKey": "wf-hello",
            "workflowVersion": 1,
            "input": {
                "name": "SES"
            },
            "state": {},
            "env": {}
        }
    });

    let response = build_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("X-Trace-Id")
            .and_then(|value| value.to_str().ok()),
        Some("trace-1")
    );

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let execute_response: ExecuteResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(execute_response.status, "success");
    assert_eq!(execute_response.output["message"], json!("Hi, SES!"));
    assert_eq!(
        execute_response.state_patch["plugins"][PLUGIN_ID]["lastGreeting"],
        json!("Hi, SES!")
    );
}

#[tokio::test]
async fn execute_route_dispatches_formal_descriptor_variants() {
    let payload = json!({
        "pluginId": FORMAL_PLUGIN_ID,
        "runnerType": FORMAL_PLUGIN_RUNNER_TYPE,
        "nodeId": "node-hello-formal-1",
        "config": {},
        "context": {
            "runId": "run-2",
            "requestId": "req-2",
            "traceId": "trace-2",
            "workflowKey": "wf-hello-formal",
            "workflowVersion": 1,
            "input": {
                "name": "SES"
            },
            "state": {},
            "env": {}
        }
    });

    let response = build_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let execute_response: ExecuteResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(execute_response.output["message"], json!("Greetings, SES."));
    assert_eq!(execute_response.output["pluginId"], json!(FORMAL_PLUGIN_ID));
    assert_eq!(
        execute_response.state_patch["plugins"][FORMAL_PLUGIN_ID]["lastGreeting"],
        json!("Greetings, SES.")
    );
}
