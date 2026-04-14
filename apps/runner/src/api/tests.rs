use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
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
    let mut workflow: Value =
        serde_json::from_str(include_str!("../../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
    let fetch_base_url = spawn_echo_http_server();
    let fetch_node = workflow["nodes"]
        .as_array_mut()
        .and_then(|nodes| nodes.iter_mut().find(|node| node["id"] == "fetch_order"))
        .expect("sorting flow should contain fetch node");
    fetch_node["config"] = json!({
        "method": "GET",
        "url": format!("{fetch_base_url}/todos")
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
                .uri("/edit-sessions")
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
    let create_payload: Value =
        serde_json::from_slice(&create_body).expect("response body should be valid json");
    let session_id = create_payload["sessionId"]
        .as_str()
        .expect("session id should be present")
        .to_string();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/edit-sessions/{session_id}"))
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
                .uri(format!("/edit-sessions/{session_id}"))
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
    let get_payload: Value =
        serde_json::from_slice(&get_body).expect("response body should be valid json");

    assert_eq!(get_payload["sessionId"], json!(session_id));
    assert_eq!(get_payload["workflowId"], json!("wf-ai-1"));
    assert_eq!(
        get_payload["workflow"]["meta"]["name"],
        json!("Updated Flow")
    );
}

fn spawn_echo_http_server() -> String {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("echo test server should bind to a random port");
    let address = listener
        .local_addr()
        .expect("echo test server should expose local address");

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];

            loop {
                let read = stream
                    .read(&mut chunk)
                    .expect("echo test server should read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            let response_body = json!({ "status": "loaded" }).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("echo test server should write response");
        }
    });

    format!("http://{address}")
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
                .uri(format!("/runs/{run_id}/terminate"))
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
    let terminate_payload: Value =
        serde_json::from_slice(&terminate_body).expect("response body should be valid json");
    assert_eq!(terminate_payload["status"], json!("terminated"));

    let terminated_summary = get_summary(app, &run_id).await;
    assert_eq!(terminated_summary["status"], json!("terminated"));
}

#[tokio::test]
async fn terminates_running_run_after_current_node_finishes() {
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
                "config": {
                    "language": "js",
                    "source": "await new Promise((resolve) => setTimeout(resolve, 150)); return { output: { done: true } };",
                    "timeoutMs": 5000
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

    let terminate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/runs/{run_id}/terminate"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(terminate_response.status(), StatusCode::OK);

    let terminated_summary = wait_for_status(app, &run_id, "terminated").await;
    assert_eq!(terminated_summary["status"], json!("terminated"));
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
                .uri("/workflows")
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
                .uri("/workflows")
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
    let list_payload: Value =
        serde_json::from_slice(&list_body).expect("response body should be valid json");
    assert_eq!(list_payload[0]["workflowId"], json!("wf-editor"));
    assert_eq!(list_payload[0]["name"], json!("Editor Backed Flow"));
    assert_eq!(list_payload[0]["status"], json!("published"));

    let detail_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/workflows/wf-editor")
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
    let detail_payload: Value =
        serde_json::from_slice(&detail_body).expect("response body should be valid json");
    assert_eq!(detail_payload["workflowId"], json!("wf-editor"));
    assert_eq!(
        detail_payload["document"]["workflow"]["version"],
        json!("v3")
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
                .uri(format!("/workflows/{workflow_id}/runs"))
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
    let execute_payload: Value =
        serde_json::from_slice(&execute_body).expect("response body should be valid json");
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
