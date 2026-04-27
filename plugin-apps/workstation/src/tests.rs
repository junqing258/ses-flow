use axum::body::{Body, to_bytes};
use axum::http::Request;
use serde_json::json;
use tower::ServiceExt;

use crate::models::{ExecutionTask, TaskErrorPayload, TaskState};
use crate::router::build_router;
use crate::services::{AppState, worker_id_from_connect};
use crate::views::{failure_resume_event, heartbeat_payload, success_resume_event};
use crate::{AppConfig, DEFAULT_CONNECT_WORKER_ID};

fn build_test_app(config: AppConfig) -> (axum::Router, AppState) {
    let state = AppState::new(config);
    (build_router(state.clone()), state)
}

#[test]
fn default_heartbeat_interval_is_shorter_than_client_read_timeout() {
    assert_eq!(AppConfig::default().heartbeat_interval_secs, 5);
}

#[tokio::test]
async fn descriptors_route_returns_manual_nodes() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(Request::builder().uri("/descriptors").body(Body::empty()).unwrap())
        .await
        .expect("descriptors request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("descriptors body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("descriptors response should be valid json");
    assert_eq!(payload.as_array().expect("descriptors should be an array").len(), 2);
    assert_eq!(payload[0]["id"], json!("scan_task"));
    assert_eq!(payload[0]["runnerType"], json!("plugin:scan_task"));
    assert_eq!(payload[0]["supportsResume"], json!(true));
    assert_eq!(payload[0]["configSchema"]["required"], json!(["stationId"]));
    assert_eq!(
        payload[0]["inputSchema"]["required"],
        json!(["orderId", "waveId", "barcode", "chuteId", "count"])
    );
    assert_eq!(payload[1]["id"], json!("pack_task"));
    assert_eq!(payload[1]["runnerType"], json!("plugin:pack_task"));
    assert_eq!(payload[1]["supportsResume"], json!(false));
    assert_eq!(payload[1]["configSchema"]["required"], json!(["stationId"]));
    assert_eq!(
        payload[1]["inputSchema"]["required"],
        json!(["chuteId", "waveId", "itemCount"])
    );
    assert_eq!(payload[0]["color"], json!("#F97316"));
    assert_eq!(payload[0]["icon"], json!("package-check"));
    assert_eq!(payload[1]["color"], json!("#14B8A6"));
    assert_eq!(payload[1]["icon"], json!("badge-check"));
}

#[tokio::test]
async fn connect_succeeds_without_authorization_header() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "ClientId": "station-1",
                        "PlatformId": "platform-1",
                        "StationIds": ["station-1"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")),
        "connect should return an SSE response"
    );
}

#[tokio::test]
async fn connect_succeeds_with_empty_payload() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("connect body should be readable");
    let body_text = String::from_utf8(body.to_vec()).expect("connect body should be utf-8");
    assert!(
        body_text.contains(DEFAULT_CONNECT_WORKER_ID),
        "empty connect should fall back to the anonymous worker id"
    );
}

#[test]
fn heartbeat_payload_uses_legacy_wcs_online_status() {
    let payload = heartbeat_payload("station-1");

    assert_eq!(payload["messageType"], json!("HEART_BEAT"));
    assert_eq!(payload["MessageType"], json!("HEART_BEAT"));
    assert_eq!(payload["RcsStatus"], json!("ONLINE"));
    assert_eq!(payload["StationList"][0]["Stationid"], json!("station-1"));
    assert_eq!(payload["StationList"][0]["StationStatus"], json!("OPEN"));
}

#[test]
fn connect_request_accepts_client_camel_case_payload_and_prefers_station_id() {
    let request: crate::models::ConnectRequest = serde_json::from_value(json!({
        "clientId": "random-client-id",
        "platformId": "platform-1",
        "stationIds": ["station-1"]
    }))
    .expect("client connect payload should deserialize");

    assert_eq!(worker_id_from_connect(&request), "station-1");
}

#[tokio::test]
async fn synchronize_returns_station_status_sync_data() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/synchronize")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "StationId": "station-1",
                        "Status": 1,
                        "PlatformId": "platform-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("synchronize request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("synchronize body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("synchronize response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Message"], json!("Success"));
    assert_eq!(payload["Data"]["StationId"], json!("station-1"));
    assert_eq!(payload["Data"]["Status"], json!(1));
    assert_eq!(payload["Data"]["PlatformId"], json!("platform-1"));
}

#[tokio::test]
async fn synchronize_defaults_to_enabled_status() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/synchronize")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .expect("synchronize request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("synchronize body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("synchronize response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Message"], json!("Success"));
    assert_eq!(payload["Data"]["Status"], json!(1));
    assert!(payload["Data"].get("status").is_none());
}

#[tokio::test]
async fn synchronize_accepts_client_camel_case_payload() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/synchronize")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "status": 0,
                        "platformId": "platform-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("synchronize request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("synchronize body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("synchronize response should be valid json");
    assert_eq!(payload["Data"]["StationId"], json!("station-1"));
    assert_eq!(payload["Data"]["Status"], json!(0));
    assert_eq!(payload["Data"]["PlatformId"], json!("platform-1"));
}

#[tokio::test]
async fn login_accepts_client_camel_case_payload() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "username": "admin",
                        "password": "123456",
                        "platformId": "platform-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("login request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("login body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("login response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Message"], json!("Success"));
    assert!(
        payload["Data"]["Authorization"]
            .as_str()
            .is_some_and(|value| value.starts_with("Bearer ")),
        "login should return a bearer token"
    );
}

#[tokio::test]
async fn robot_departure_completes_active_task() {
    let (app, state) = build_test_app(AppConfig::default());

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "StationId": "station-1",
                        "PlatformId": "platform-1",
                        "Username": "demo",
                        "Password": "demo"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("login request should succeed");
    let login_body = to_bytes(login_response.into_body(), usize::MAX)
        .await
        .expect("login body should be readable");
    let login_payload: serde_json::Value =
        serde_json::from_slice(&login_body).expect("login payload should be valid json");
    let authorization = login_payload["Data"]["Authorization"]
        .as_str()
        .expect("authorization token should exist");

    let execute_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pluginId": "scan_task",
                        "runnerType": "plugin:scan_task",
                        "nodeId": "scan_task_1",
                        "config": {
                            "stationId": "station-1",
                            "taskId": "TASK-1"
                        },
                        "context": {
                            "runId": "run-1",
                            "requestId": "req-1",
                            "workflowKey": "workflow.demo",
                            "workflowVersion": 1,
                            "input": {
                                "orderNo": "SO-1"
                            },
                            "state": {},
                            "env": {}
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("execute request should succeed");
    let execute_body = to_bytes(execute_response.into_body(), usize::MAX)
        .await
        .expect("execute body should be readable");
    let execute_payload: serde_json::Value =
        serde_json::from_slice(&execute_body).expect("execute payload should be valid json");
    assert_eq!(execute_payload["status"], json!("waiting"));
    let execution_id = execute_payload["output"]["executionId"]
        .as_str()
        .expect("execution id should exist")
        .to_string();

    let depart_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/robotDeparture")
                .header("content-type", "application/json")
                .header("authorization", authorization)
                .body(Body::from(
                    json!({
                        "TaskId": "TASK-1",
                        "AgvId": "AGV-1",
                        "Completed": 1,
                        "RequestId": "agv-req-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("robotDeparture request should succeed");
    assert_eq!(depart_response.status(), axum::http::StatusCode::OK);

    let state = state.inner.read().await;
    let task = state.tasks.get(&execution_id).expect("task should exist after execute");
    assert!(matches!(task.state, TaskState::Succeeded));
}

#[test]
fn builds_runner_resume_events() {
    let task = ExecutionTask {
        execution_id: "exec-1".to_string(),
        run_id: "run-1".to_string(),
        request_id: "req-1".to_string(),
        node_id: "node-1".to_string(),
        trace_id: None,
        plugin_type: "plugin:scan_task".to_string(),
        plugin_id: "scan_task".to_string(),
        target_worker_id: "station-1".to_string(),
        payload: json!({}),
        task_id: "TASK-1".to_string(),
        wait_signal_type: "human_task_done".to_string(),
        state: TaskState::Pending,
        runner_base_url: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now(),
    };
    let success = success_resume_event(
        &task,
        json!({ "taskId": "TASK-1" }),
        json!({ "workstation": { "status": "done" } }),
    );
    let failure = failure_resume_event(
        &task,
        &TaskErrorPayload {
            code: "SCAN_FAILED".to_string(),
            message: "扫码超时".to_string(),
        },
    );

    assert_eq!(success["type"], json!("human_task_done"));
    assert_eq!(success["payload"]["output"]["taskId"], json!("TASK-1"));
    assert_eq!(failure["payload"]["status"], json!("failed"));
    assert_eq!(failure["payload"]["error"]["code"], json!("SCAN_FAILED"));
}
