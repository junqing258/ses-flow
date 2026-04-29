use axum::body::{Body, to_bytes};
use axum::http::Request;
use serde_json::json;
use tower::ServiceExt;

use crate::models::{ExecutionTask, TaskErrorPayload, TaskState};
use crate::router::build_router;
use crate::services::{AppState, station_id_from_connect};
use crate::views::{failure_resume_event, heartbeat_payload, success_resume_event};
use crate::{AppConfig, DEFAULT_CONNECT_STATION_ID};

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
    let descriptors = payload.as_array().expect("descriptors should be an array");
    assert!(descriptors.len() >= 2);
    let scan_task = descriptors
        .iter()
        .find(|item| item["id"] == json!("scan_task"))
        .expect("scan_task descriptor should exist");
    let pack_task = descriptors
        .iter()
        .find(|item| item["id"] == json!("pack_task"))
        .expect("pack_task descriptor should exist");
    let get_task_info = descriptors
        .iter()
        .find(|item| item["id"] == json!("get_task_info"))
        .expect("get_task_info descriptor should exist");
    let robot_departure = descriptors
        .iter()
        .find(|item| item["id"] == json!("robot_departure"))
        .expect("robot_departure descriptor should exist");
    assert_eq!(scan_task["runnerType"], json!("plugin:scan_task"));
    assert_eq!(scan_task["supportsResume"], json!(true));
    assert_eq!(scan_task["configSchema"]["required"], json!(["stationId"]));
    assert_eq!(scan_task["inputSchema"]["required"], json!(["agvId"]));
    assert_eq!(pack_task["runnerType"], json!("plugin:pack_task"));
    assert_eq!(pack_task["supportsResume"], json!(false));
    assert_eq!(pack_task["configSchema"]["required"], json!(["stationId"]));
    assert_eq!(
        pack_task["inputSchema"]["required"],
        json!(["chuteId", "waveId", "itemCount"])
    );
    assert_eq!(scan_task["color"], json!("#F97316"));
    assert_eq!(scan_task["icon"], json!("scan-barcode"));
    assert_eq!(pack_task["color"], json!("#14B8A6"));
    assert_eq!(pack_task["icon"], json!("badge-check"));
    assert!(
        get_task_info["configSchema"].get("required").is_none(),
        "SES base URL should be allowed to come from the execution context"
    );
    assert!(
        get_task_info["configSchema"]["properties"].get("sesBaseUrl").is_none(),
        "get_task_info should not expose SES base URL as plugin config"
    );
    assert!(
        robot_departure["configSchema"]["properties"]
            .get("sesBaseUrl")
            .is_none(),
        "robot_departure should not expose SES base URL as plugin config"
    );
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
        body_text.contains(DEFAULT_CONNECT_STATION_ID),
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

    assert_eq!(station_id_from_connect(&request), "station-1");
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
async fn scan_barcode_accepts_client_lowercase_payload_and_returns_items() {
    let (app, _) = build_test_app(AppConfig::default());
    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "username": "demo",
                        "password": "demo",
                        "platformId": "platform-1"
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

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/scanBarcode")
                .header("content-type", "application/json")
                .header("authorization", authorization)
                .body(Body::from(json!({ "barcode": "123" }).to_string()))
                .unwrap(),
        )
        .await
        .expect("scan barcode request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("scan barcode body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("scan barcode response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Data"]["Items"][0]["BarCode"], json!("123"));
    assert_eq!(payload["Data"]["Items"][0]["BarcodeName"], json!("商品-123"));
    assert_eq!(payload["Data"]["Items"][0]["Sku"], json!("SKU-123"));
    assert_eq!(payload["Data"]["Items"][0]["ItemId"], json!("123"));
    assert_eq!(payload["Data"]["ResumedRunIds"], json!([]));
}

#[tokio::test]
async fn scan_barcode_falls_back_to_connected_worker_for_simulation_auth() {
    let (app, _) = build_test_app(AppConfig::default());
    let connect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "stationIds": ["station-1"] }).to_string()))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");
    assert_eq!(connect_response.status(), axum::http::StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/scanBarcode")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "barcode": "123" }).to_string()))
                .unwrap(),
        )
        .await
        .expect("scan barcode request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("scan barcode body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("scan barcode response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Data"]["WorkerId"], json!("station-1"));
    assert_eq!(payload["Data"]["Items"][0]["BarCode"], json!("123"));
    assert_eq!(payload["Data"]["ResumedRunIds"], json!([]));
}

#[tokio::test]
async fn get_task_info_returns_mock_task_without_active_runner_task() {
    let (app, _) = build_test_app(AppConfig::default());
    let connect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "stationIds": ["station-1"] }).to_string()))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");
    assert_eq!(connect_response.status(), axum::http::StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/getTaskInfo")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "sku": "SKU-123",
                        "barcode": "123",
                        "completed": 1,
                        "waveType": "ORDER",
                        "parcelId": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("get task info request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("get task info body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("get task info response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Data"]["TaskId"], json!("TASK-SKU-123"));
    assert_eq!(payload["Data"]["ChuteId"], json!("C-SKU"));
    assert_eq!(payload["Data"]["WaveId"], json!("ORDER"));
    assert_eq!(payload["Data"]["OrderId"], json!("ORDER-station-1"));
    assert_eq!(payload["Data"]["Count"], json!(2));
}

#[tokio::test]
async fn get_task_info_plugin_execute_returns_success_without_waiting_task() {
    let (app, state) = build_test_app(AppConfig::default());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pluginId": "get_task_info",
                        "runnerType": "plugin:get_task_info",
                        "nodeId": "get_task_info_2",
                        "config": {},
                        "context": {
                            "runId": "run-1",
                            "requestId": "req-1",
                            "workflowKey": "workflow.demo",
                            "workflowVersion": 1,
                            "input": {
                                "stationId": "station-1",
                                "sku": "SKU-123",
                                "itemId": "123",
                                "agvId": "AGV-1"
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
        .expect("get_task_info execute request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("get_task_info execute body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("get_task_info execute response should be valid json");
    assert_eq!(payload["status"], json!("success"));
    assert_eq!(payload["output"]["taskId"], json!("TASK-SKU-123"));
    assert_eq!(payload["output"]["targetId"], json!("C-SKU"));
    assert_eq!(payload["output"]["agvId"], json!("AGV-1"));
    assert!(payload.get("waitSignal").is_none());
    assert!(state.inner.read().await.tasks.is_empty());
}

#[tokio::test]
async fn station_status_routes_accept_client_empty_payloads() {
    let (app, _) = build_test_app(AppConfig::default());
    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "username": "demo",
                        "password": "demo",
                        "platformId": "platform-1"
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

    for (uri, expected_key) in [
        ("/station/operation/offline", "Online"),
        ("/station/operation/online", "Online"),
        ("/station/operation/logout", "LoggedOut"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .header("authorization", authorization)
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .expect("station status request should succeed");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("station status body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("station status response should be valid json");
        assert_eq!(payload["Code"], json!(0));
        assert_eq!(payload["Data"]["StationId"], json!("station-1"));
        assert!(payload["Data"].get(expected_key).is_some());
    }
}

#[tokio::test]
async fn robot_departure_succeeds_without_active_runner_task_for_simulation() {
    let (app, _) = build_test_app(AppConfig::default());
    let connect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "stationIds": ["station-1"] }).to_string()))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");
    assert_eq!(connect_response.status(), axum::http::StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/robotDeparture")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "taskId": "TASK-SKU-123",
                        "agvId": "AGV-001",
                        "completed": 1,
                        "requestId": "8128831acda14b9220260427162429769"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("robot departure request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("robot departure body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("robot departure response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Data"]["TaskId"], json!("TASK-SKU-123"));
    assert_eq!(payload["Data"]["AgvId"], json!("AGV-001"));
    assert_eq!(payload["Data"]["ResumedRunIds"], json!([]));
}

#[tokio::test]
async fn robot_departure_plugin_consumes_early_legacy_departure_call() {
    let (app, state) = build_test_app(AppConfig::default());
    let connect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "stationIds": ["station-1"] }).to_string()))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");
    assert_eq!(connect_response.status(), axum::http::StatusCode::OK);

    let depart_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/robotDeparture")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "taskId": "TASK-SKU-123",
                        "agvId": "AGV-001",
                        "completed": 1,
                        "requestId": "early-departure-1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("robotDeparture request should succeed");
    assert_eq!(depart_response.status(), axum::http::StatusCode::OK);

    let execute_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pluginId": "robot_departure",
                        "runnerType": "plugin:robot_departure",
                        "nodeId": "robot_departure",
                        "config": {},
                        "context": {
                            "runId": "run-1",
                            "requestId": "req-1",
                            "workflowKey": "workflow.demo",
                            "workflowVersion": 1,
                            "input": {
                                "stationId": "station-1",
                                "taskId": "TASK-SKU-123",
                                "agvId": "AGV-001"
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
        .expect("robot_departure execute request should succeed");

    assert_eq!(execute_response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(execute_response.into_body(), usize::MAX)
        .await
        .expect("robot_departure execute body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("robot_departure execute response should be valid json");
    assert_eq!(payload["status"], json!("success"));
    assert_eq!(payload["output"]["taskId"], json!("TASK-SKU-123"));
    assert_eq!(payload["output"]["result"], json!(0));
    assert!(payload.get("waitSignal").is_none());
    assert!(state.inner.read().await.tasks.is_empty());
}

#[tokio::test]
async fn driver_empty_robot_alias_accepts_legacy_java_payload() {
    let (app, _) = build_test_app(AppConfig::default());
    let connect_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "stationIds": ["station-1"] }).to_string()))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");
    assert_eq!(connect_response.status(), axum::http::StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/driverEmptyRobot")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "agvId": "AGV-001"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("driverEmptyRobot request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("driverEmptyRobot body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("driverEmptyRobot response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Data"]["StationId"], json!("station-1"));
    assert_eq!(payload["Data"]["AgvId"], json!("AGV-001"));
    assert_eq!(payload["Data"]["Forced"], json!(true));
}

#[tokio::test]
async fn simulate_agv_arrived_queues_legacy_sse_event() {
    let (app, _) = build_test_app(AppConfig::default());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/simulate/agvArrived")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "agvId": "AGV-9",
                        "requestId": 42
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("simulate AGV arrival request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("simulate AGV arrival body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("simulate AGV arrival response should be valid json");
    assert_eq!(payload["Code"], json!(0));
    assert_eq!(payload["Data"]["MessageType"], json!("AGV_ARRIVED"));
    assert_eq!(payload["Data"]["RequestId"], json!("42"));

    let connect_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/connect")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationIds": ["station-1"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("connect request should succeed");
    let connect_body = to_bytes(connect_response.into_body(), usize::MAX)
        .await
        .expect("connect body should be readable");
    let connect_text = String::from_utf8(connect_body.to_vec()).expect("connect body should be utf-8");

    assert!(connect_text.contains("\"messageType\":\"AGV_ARRIVED\""));
    assert!(connect_text.contains("\"AgvId\":\"AGV-9\""));
    assert!(connect_text.contains("\"StationId\":\"station-1\""));
    assert!(connect_text.contains("\"RequestId\":42"));
}

#[tokio::test]
async fn simulate_agv_arrived_skips_runner_resume_without_database_url() {
    let (app, _) = build_test_app(AppConfig {
        runner_base_url: Some("http://127.0.0.1:1/runner-api".to_string()),
        ses_auth_base_url: None,
        database_url: None,
        heartbeat_interval_secs: 5,
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/station/operation/simulate/agvArrived")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "agvId": "AGV-9"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("simulate AGV arrival request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("simulate AGV arrival body should be readable");
    let payload: serde_json::Value =
        serde_json::from_slice(&body).expect("simulate AGV arrival response should be valid json");
    assert_eq!(payload["Data"]["ResumedRunIds"], json!([]));
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
                        "pluginId": "robot_departure",
                        "runnerType": "plugin:robot_departure",
                        "nodeId": "robot_departure",
                        "config": {
                            "stationId": "station-1"
                        },
                        "context": {
                            "runId": "run-1",
                            "requestId": "req-1",
                            "workflowKey": "workflow.demo",
                            "workflowVersion": 1,
                            "input": {
                                "taskId": "TASK-1"
                            },
                            "state": {},
                            "env": {
                                "sesBaseUrl": "http://ses.example/station/operation"
                            }
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
    assert_eq!(
        task.payload["sesBaseUrl"],
        json!("http://ses.example/station/operation")
    );
}

#[tokio::test]
async fn robot_departure_does_not_complete_non_departure_active_task() {
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
                            "input": {},
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
    assert!(matches!(task.state, TaskState::Pending));
}

#[tokio::test]
async fn execute_resolves_station_id_from_workstation_state() {
    let (app, _) = build_test_app(AppConfig::default());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/execute")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "pluginId": "robot_departure",
                        "runnerType": "plugin:robot_departure",
                        "nodeId": "robot_departure",
                        "config": {},
                        "context": {
                            "runId": "run-1",
                            "requestId": "req-robot-1",
                            "workflowKey": "workflow.demo",
                            "workflowVersion": 1,
                            "input": {
                                "taskId": "TASK-SKU-123",
                                "agvId": "AGV-001"
                            },
                            "state": {
                                "workstation": {
                                    "executions": {
                                        "exec-1": {
                                            "status": "pending",
                                            "taskId": "exec-1",
                                            "workerId": "station-1"
                                        }
                                    }
                                }
                            },
                            "env": {}
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("execute request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("execute body should be readable");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("execute payload should be valid json");
    assert_eq!(payload["status"], json!("waiting"));
    assert_eq!(payload["output"]["workerId"], json!("station-1"));
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
        target_station_id: "station-1".to_string(),
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
