use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tower::ServiceExt;

use runner::app::WorkflowApp;
use runner::store::{InMemoryRunStore, WorkflowRunStore};

use crate::modules::{ApiState, RUNNER_API_BASE_PATH, build_router};

fn build_app() -> axum::Router {
    build_app_with_ai_gateway_target("http://127.0.0.1:6307")
}

fn build_app_with_ai_gateway_target(target: &str) -> axum::Router {
    build_router(ApiState {
        app: Arc::new(WorkflowApp::new()),
        ai_gateway_base_url: target.to_string(),
        ai_gateway_client: reqwest::Client::new(),
    })
}

fn build_app_with_server(app: Arc<WorkflowApp>) -> axum::Router {
    build_router(ApiState {
        app,
        ai_gateway_base_url: "http://127.0.0.1:6307".to_string(),
        ai_gateway_client: reqwest::Client::new(),
    })
}

fn api_path(path: &str) -> String {
    format!("{RUNNER_API_BASE_PATH}{path}")
}

fn spawn_delayed_http_server(delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("delayed test server should bind to a random port");
    let address = listener
        .local_addr()
        .expect("delayed test server should expose local address");

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };

            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            thread::sleep(delay);

            let response_body = json!({ "status": "slow" }).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    format!("http://{address}")
}

struct CapturedHttpRequest {
    request_line: String,
    raw_headers: String,
    body: Vec<u8>,
}

fn spawn_single_response_http_server(response: String) -> (String, mpsc::Receiver<CapturedHttpRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("proxy test server should bind to a random port");
    let address = listener
        .local_addr()
        .expect("proxy test server should expose local address");
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("proxy test server should accept a request");

        let request = read_http_request(&mut stream);
        sender.send(request).expect("captured proxy request should be sent");
        stream
            .write_all(response.as_bytes())
            .expect("proxy test server should write response");
    });

    (format!("http://{address}"), receiver)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> CapturedHttpRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    let mut headers_end = None;
    let mut content_length = 0usize;

    loop {
        let bytes_read = stream.read(&mut chunk).expect("proxy test server should read request");
        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..bytes_read]);

        if headers_end.is_none() {
            headers_end = find_header_end(&buffer);
            if let Some(position) = headers_end {
                content_length = parse_content_length(&buffer[..position]);
            }
        }

        if let Some(position) = headers_end {
            if buffer.len() >= position + content_length {
                break;
            }
        }
    }

    let headers_end = headers_end.expect("proxy request should include headers");
    let request_head = String::from_utf8_lossy(&buffer[..headers_end]).to_string();
    let mut lines = request_head.lines();
    let request_line = lines.next().unwrap_or_default().to_string();
    let raw_headers = lines.collect::<Vec<_>>().join("\n");

    CapturedHttpRequest {
        request_line,
        raw_headers,
        body: buffer[headers_end..].to_vec(),
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn parse_content_length(headers: &[u8]) -> usize {
    String::from_utf8_lossy(headers)
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<usize>().ok();
            }
            None
        })
        .unwrap_or(0)
}

#[tokio::test]
async fn adds_cors_headers_to_json_responses() {
    let app = build_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/health"))
                .header(header::ORIGIN, "http://localhost:5173")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("cors header should be present"),
        "*"
    );
}

#[tokio::test]
async fn redirects_root_to_views() {
    let app = build_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok()),
        Some("/views/")
    );
}

#[tokio::test]
async fn handles_cors_preflight_requests() {
    let app = build_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri(api_path("/workflows"))
                .header(header::ORIGIN, "http://localhost:5173")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type,x-request-id")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert!(response.status().is_success());
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("allow origin should be present"),
        "*"
    );
    assert!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("POST")),
        "preflight should advertise POST support"
    );
    assert!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == "*" || value.to_ascii_lowercase().contains("content-type")),
        "preflight should allow requested headers"
    );
}

#[tokio::test]
async fn proxies_ai_gateway_json_requests() {
    let response_body = json!({
        "status": "accepted",
        "source": "ai-gateway"
    })
    .to_string();
    let upstream_response = format!(
        "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nX-Upstream: ai-gateway\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    let (server_url, captured_request_receiver) = spawn_single_response_http_server(upstream_response);
    let app = build_app_with_ai_gateway_target(&server_url);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ai/threads/session-1/messages?draft=1")
                .header("content-type", "application/json")
                .header("accept", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "message": "hello proxy"
                    }))
                    .expect("request body should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("proxy request should succeed");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("x-upstream")
            .and_then(|value| value.to_str().ok()),
        Some("ai-gateway")
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("proxy response body should collect")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body).expect("proxy response should be valid json");
    assert_eq!(payload["status"], json!("accepted"));

    let captured = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("upstream server should capture the proxied request");
    assert_eq!(
        captured.request_line,
        "POST /api/ai/threads/session-1/messages?draft=1 HTTP/1.1"
    );
    assert!(
        captured
            .raw_headers
            .to_ascii_lowercase()
            .contains("content-type: application/json"),
        "proxy should preserve content-type for upstream"
    );
    assert_eq!(
        String::from_utf8(captured.body).expect("request body should be valid utf-8"),
        r#"{"message":"hello proxy"}"#
    );
}

#[tokio::test]
async fn proxies_ai_gateway_sse_responses() {
    let sse_payload = "event: thread.snapshot\ndata: {\"status\":\"idle\"}\n\n";
    let upstream_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        sse_payload.len(),
        sse_payload
    );
    let (server_url, captured_request_receiver) = spawn_single_response_http_server(upstream_response);
    let app = build_app_with_ai_gateway_target(&server_url);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ai/threads/session-1/events")
                .header("accept", "text/event-stream")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("proxy request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache")
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("sse response body should collect")
        .to_bytes();
    assert_eq!(
        String::from_utf8(body.to_vec()).expect("sse body should be valid utf-8"),
        sse_payload
    );

    let captured = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("upstream server should capture the proxied sse request");
    assert_eq!(captured.request_line, "GET /api/ai/threads/session-1/events HTTP/1.1");
    assert!(
        captured
            .raw_headers
            .to_ascii_lowercase()
            .contains("accept: text/event-stream"),
        "proxy should preserve sse accept header for upstream"
    );
}

#[tokio::test]
async fn refreshes_catalog_via_get_endpoint() {
    let app = build_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/catalog/refresh"))
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
    let payload: Value = serde_json::from_slice(&body).expect("response body should be valid json");
    assert_eq!(payload["status"], json!("ok"));
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
                .uri(api_path("/workflows"))
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
    let upload_payload: Value = serde_json::from_slice(&upload_body).expect("response body should be valid json");
    let workflow_id = upload_payload["workflowId"]
        .as_str()
        .expect("workflow id should be present")
        .to_string();

    let execute_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/workflows/{workflow_id}/run")))
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
    let execute_payload: Value = serde_json::from_slice(&execute_body).expect("response body should be valid json");
    let run_id = execute_payload["runId"]
        .as_str()
        .expect("run id should be present")
        .to_string();

    let summary = wait_for_terminal_status(app, &run_id).await;
    assert_eq!(summary["status"], json!("completed"));
    assert_eq!(summary["workflowKey"], json!("api-server-flow"));
}

#[tokio::test]
async fn creates_and_updates_edit_session_draft() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "edit-session-flow",
            "name": "Edit Session Flow",
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

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path("/edit-sessions"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "workspaceId": "ws-ai",
                        "workflowId": "wf-ai-1",
                        "editorDocument": {
                            "schemaVersion": "1.0"
                        },
                        "workflow": workflow
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = create_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let create_payload: Value = serde_json::from_slice(&create_body).expect("response body should be valid json");
    let session_id = create_payload["sessionId"]
        .as_str()
        .expect("session id should be present")
        .to_string();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(api_path(&format!("/edit-sessions/{session_id}/draft")))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "workflowId": "wf-ai-1",
                        "editorDocument": {
                            "schemaVersion": "1.0",
                            "workflow": {
                                "name": "Updated Flow"
                            }
                        },
                        "workflow": {
                            "meta": {
                                "key": "edit-session-flow",
                                "name": "Updated Flow",
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
                        }
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(update_response.status(), StatusCode::OK);

    let get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path(&format!("/edit-sessions/{session_id}")))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = get_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let get_payload: Value = serde_json::from_slice(&get_body).expect("response body should be valid json");

    assert_eq!(get_payload["sessionId"], json!(session_id));
    assert_eq!(get_payload["workflowId"], json!("wf-ai-1"));
    assert_eq!(get_payload["workflow"]["meta"]["name"], json!("Updated Flow"));
}

#[tokio::test]
async fn patches_edit_session_draft_with_remove_node_cascade_operation() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "edit-session-flow",
            "name": "Edit Session Flow",
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
            { "id": "fetch_1", "type": "fetch", "name": "Fetch" },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "fetch_1" },
            { "from": "fetch_1", "to": "end_1" }
        ],
        "policies": {}
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path("/edit-sessions"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "workspaceId": "ws-ai",
                        "workflowId": "wf-ai-1",
                        "editorDocument": {
                            "schemaVersion": "1.0",
                            "editor": {
                                "selectedNodeId": "fetch_1"
                            },
                            "graph": {
                                "nodes": [
                                    { "id": "start_1", "type": "terminal" },
                                    { "id": "fetch_1", "type": "workflow-card" },
                                    { "id": "end_1", "type": "terminal" }
                                ],
                                "edges": [
                                    { "id": "edge:start->fetch", "source": "start_1", "target": "fetch_1" },
                                    { "id": "edge:fetch->end", "source": "fetch_1", "target": "end_1" }
                                ],
                                "panels": {
                                    "start_1": { "tabs": ["base"], "fieldsByTab": {} },
                                    "fetch_1": { "tabs": ["base"], "fieldsByTab": {} },
                                    "end_1": { "tabs": ["base"], "fieldsByTab": {} }
                                }
                            },
                            "workflow": {
                                "id": "wf-ai-1",
                                "name": "Edit Session Flow",
                                "status": "draft",
                                "version": "1"
                            }
                        },
                        "workflow": workflow
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = create_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let create_payload: Value = serde_json::from_slice(&create_body).expect("response body should be valid json");
    let session_id = create_payload["sessionId"]
        .as_str()
        .expect("session id should be present")
        .to_string();

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(api_path(&format!("/edit-sessions/{session_id}/draft")))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "operations": [
                            {
                                "type": "remove_node_cascade",
                                "nodeId": "fetch_1"
                            }
                        ]
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(patch_response.status(), StatusCode::OK);
    let patch_body = patch_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let patch_payload: Value = serde_json::from_slice(&patch_body).expect("response body should be valid json");

    assert_eq!(
        patch_payload["workflow"]["nodes"]
            .as_array()
            .expect("workflow nodes should exist")
            .iter()
            .filter_map(|node| node["id"].as_str())
            .collect::<Vec<_>>(),
        vec!["start_1", "end_1"]
    );
    assert_eq!(
        patch_payload["workflow"]["transitions"]
            .as_array()
            .expect("workflow transitions should exist")
            .iter()
            .map(|transition| (
                transition["from"].as_str().expect("transition from should exist"),
                transition["to"].as_str().expect("transition to should exist"),
            ))
            .collect::<Vec<_>>(),
        vec![("start_1", "end_1")]
    );
    assert_eq!(
        patch_payload["editorDocument"]["graph"]["nodes"]
            .as_array()
            .expect("editor document nodes should exist")
            .iter()
            .filter_map(|node| node["id"].as_str())
            .collect::<Vec<_>>(),
        vec!["start_1", "end_1"]
    );
    assert!(
        patch_payload["editorDocument"]["graph"]["edges"]
            .as_array()
            .expect("editor document edges should exist")
            .iter()
            .any(|edge| { edge["source"].as_str() == Some("start_1") && edge["target"].as_str() == Some("end_1") })
    );
    assert!(patch_payload["editorDocument"]["graph"]["panels"]["fetch_1"].is_null());
    assert!(patch_payload["editorDocument"]["editor"]["selectedNodeId"].is_null());
}

#[tokio::test]
async fn terminates_waiting_run() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "terminate-waiting-flow",
            "name": "Terminate Waiting Flow",
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
            {
                "id": "wait_1",
                "type": "wait",
                "name": "Wait",
                "config": { "event": "done" }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "wait_1" },
            { "from": "wait_1", "to": "end_1" }
        ],
        "policies": {}
    });

    let workflow_id = upload_workflow(app.clone(), workflow).await;
    let run_id = start_run(app.clone(), &workflow_id, json!({})).await;

    let waiting_summary = wait_for_status(app.clone(), &run_id, "waiting").await;
    assert_eq!(waiting_summary["status"], json!("waiting"));

    let terminate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/runs/{run_id}/terminate")))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(terminate_response.status(), StatusCode::OK);
    let terminate_body = terminate_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let terminate_payload: Value = serde_json::from_slice(&terminate_body).expect("response body should be valid json");
    assert_eq!(terminate_payload["status"], json!("terminated"));

    let terminated_summary = get_summary(app, &run_id).await;
    assert_eq!(terminated_summary["status"], json!("terminated"));
}

#[tokio::test]
async fn terminates_running_code_run_without_waiting_for_current_node() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "terminate-running-flow",
            "name": "Terminate Running Flow",
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
            {
                "id": "run_code",
                "type": "code",
                "name": "Run Code",
                "timeoutMs": 5000,
                "config": {
                    "language": "js",
                    "source": "await new Promise((resolve) => setTimeout(resolve, 5000)); return { output: { done: true } };"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "run_code" },
            { "from": "run_code", "to": "end_1" }
        ],
        "policies": {}
    });

    let workflow_id = upload_workflow(app.clone(), workflow).await;
    let run_id = start_run(app.clone(), &workflow_id, json!({})).await;

    for _ in 0..40 {
        let summary = get_summary(app.clone(), &run_id).await;
        if summary["currentNodeId"] == json!("run_code") {
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }

    let started_at = Instant::now();

    let terminate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/runs/{run_id}/terminate")))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(terminate_response.status(), StatusCode::OK);

    let terminated_summary = wait_for_status(app, &run_id, "terminated").await;
    assert_eq!(terminated_summary["status"], json!("terminated"));
    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "running code node should terminate promptly once cancellation is requested",
    );
    assert_eq!(terminated_summary["currentNodeId"], json!("run_code"));
}

#[tokio::test]
async fn terminates_running_fetch_run_without_waiting_for_response() {
    let app = build_app();
    let server_url = spawn_delayed_http_server(Duration::from_secs(5));
    let workflow = json!({
        "meta": {
            "key": "terminate-running-fetch-flow",
            "name": "Terminate Running Fetch Flow",
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
            {
                "id": "fetch_1",
                "type": "fetch",
                "name": "Fetch",
                "timeoutMs": 10000,
                "config": {
                    "method": "GET",
                    "url": format!("{server_url}/slow")
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "fetch_1" },
            { "from": "fetch_1", "to": "end_1" }
        ],
        "policies": {}
    });

    let workflow_id = upload_workflow(app.clone(), workflow).await;
    let run_id = start_run(app.clone(), &workflow_id, json!({})).await;

    for _ in 0..40 {
        let summary = get_summary(app.clone(), &run_id).await;
        if summary["currentNodeId"] == json!("fetch_1") {
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }

    let started_at = Instant::now();
    let terminate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/runs/{run_id}/terminate")))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(terminate_response.status(), StatusCode::OK);

    let terminated_summary = wait_for_status(app, &run_id, "terminated").await;
    assert_eq!(terminated_summary["status"], json!("terminated"));
    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "running fetch node should terminate promptly once cancellation is requested",
    );
    assert_eq!(terminated_summary["currentNodeId"], json!("fetch_1"));
}

#[tokio::test]
async fn terminates_orphaned_running_run_immediately() {
    let store = Arc::new(InMemoryRunStore::new());
    store
        .save_summary(&json_to_summary(json!({
            "runId": "run-orphan-1",
            "workflowKey": "orphaned-flow",
            "workflowVersion": 1,
            "status": "running",
            "currentNodeId": "switch_biz_type",
            "state": {},
            "timeline": [
                {
                    "nodeId": "start",
                    "nodeType": "start",
                    "status": "success",
                    "output": {},
                    "statePatch": null
                }
            ]
        })))
        .expect("orphaned running summary should seed");

    let app = build_app_with_server(Arc::new(WorkflowApp::with_store(store.clone())));

    let terminate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path("/runs/run-orphan-1/terminate"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(terminate_response.status(), StatusCode::OK);
    let terminate_body = terminate_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let terminate_payload: Value = serde_json::from_slice(&terminate_body).expect("response body should be valid json");
    assert_eq!(terminate_payload["status"], json!("terminated"));

    let terminated_summary = store
        .load_summary("run-orphan-1")
        .expect("summary lookup should succeed")
        .expect("summary should exist");
    assert!(matches!(
        terminated_summary.status,
        runner::core::runtime::WorkflowRunStatus::Terminated
    ));
}

#[tokio::test]
async fn lists_workflows_and_returns_editor_document_for_detail() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "editor-backed-flow",
            "name": "Editor Backed Flow",
            "version": 3,
            "status": "published"
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
    let editor_document = json!({
        "schemaVersion": "1.0",
        "workflow": {
            "id": "editor-backed-flow",
            "name": "Editor Backed Flow",
            "status": "published",
            "version": "v3"
        },
        "editor": {
            "activeTab": "base",
            "selectedNodeId": "start_1"
        },
        "graph": {
            "nodes": [],
            "edges": [],
            "panels": {}
        }
    });

    let upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path("/workflows"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "workspaceId": "ws-editor",
                        "workspaceName": "Editor Workspace",
                        "workflowId": "wf-editor",
                        "editorDocument": editor_document,
                        "workflow": workflow
                    }))
                    .expect("request should serialize"),
                ))
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(upload_response.status(), StatusCode::OK);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/workflows"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = list_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let list_payload: Value = serde_json::from_slice(&list_body).expect("response body should be valid json");
    assert_eq!(list_payload[0]["workflowId"], json!("wf-editor"));
    assert_eq!(list_payload[0]["name"], json!("Editor Backed Flow"));
    assert_eq!(list_payload[0]["status"], json!("published"));

    let detail_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/workflows/wf-editor"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = detail_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let detail_payload: Value = serde_json::from_slice(&detail_body).expect("response body should be valid json");
    assert_eq!(detail_payload["workflowId"], json!("wf-editor"));
    assert_eq!(detail_payload["document"]["workflow"]["version"], json!("v3"));
}

#[tokio::test]
async fn executes_sub_workflow_references_from_registered_workflow_ids() {
    let app = build_app();
    let child_workflow = json!({
        "meta": {
            "key": "child-flow",
            "name": "Child Flow",
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
    let child_workflow_id = upload_workflow(app.clone(), child_workflow).await;

    let parent_workflow = json!({
        "meta": {
            "key": "parent-flow",
            "name": "Parent Flow",
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
            {
                "id": "nested_workflow",
                "type": "sub_workflow",
                "name": "Nested Workflow",
                "config": {
                    "ref": child_workflow_id
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "nested_workflow" },
            { "from": "nested_workflow", "to": "end_1" }
        ],
        "policies": {}
    });
    let parent_workflow_id = upload_workflow(app.clone(), parent_workflow).await;

    let run_id = start_run(app.clone(), &parent_workflow_id, json!({})).await;
    let summary = wait_for_terminal_status(app, &run_id).await;

    assert_eq!(summary["status"], json!("completed"));
    assert!(
        summary["timeline"].as_array().is_some_and(|timeline| timeline
            .iter()
            .any(|item| { item["nodeId"] == json!("nested_workflow") && item["status"] == json!("success") })),
        "parent run timeline should contain a successful sub-workflow node execution",
    );
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

async fn upload_workflow(app: axum::Router, workflow: Value) -> String {
    let upload_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path("/workflows"))
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
    let upload_payload: Value = serde_json::from_slice(&upload_body).expect("response body should be valid json");
    upload_payload["workflowId"]
        .as_str()
        .expect("workflow id should be present")
        .to_string()
}

async fn start_run(app: axum::Router, workflow_id: &str, trigger: Value) -> String {
    let execute_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/workflows/{workflow_id}/run")))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "trigger": trigger
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
    let execute_payload: Value = serde_json::from_slice(&execute_body).expect("response body should be valid json");
    execute_payload["runId"]
        .as_str()
        .expect("run id should be present")
        .to_string()
}

async fn get_summary(app: axum::Router, run_id: &str) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path(&format!("/runs/{run_id}")))
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

fn json_to_summary(value: Value) -> runner::core::runtime::WorkflowRunSummary {
    serde_json::from_value(value).expect("summary json should deserialize")
}
