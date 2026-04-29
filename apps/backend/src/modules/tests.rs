use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
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

use crate::modules::auth::{AuthService, InMemoryAuthStore};
use crate::modules::node_registry::register_http_plugin_base_urls;
use crate::modules::system::system_store::InMemorySystemSettingsStore;
use crate::modules::{ApiState, RUNNER_API_BASE_PATH, build_router};

fn build_app() -> axum::Router {
    build_app_with_ai_gateway_target("http://127.0.0.1:6307")
}

fn build_app_with_ai_gateway_target(target: &str) -> axum::Router {
    build_router(ApiState {
        app: Arc::new(WorkflowApp::new()),
        ai_gateway_base_url: target.to_string(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings: Arc::new(InMemorySystemSettingsStore::new()),
        auth: AuthService::new(Arc::new(InMemoryAuthStore::default())),
        auth_required: false,
    })
}

fn build_app_with_server(app: Arc<WorkflowApp>) -> axum::Router {
    build_router(ApiState {
        app,
        ai_gateway_base_url: "http://127.0.0.1:6307".to_string(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings: Arc::new(InMemorySystemSettingsStore::new()),
        auth: AuthService::new(Arc::new(InMemoryAuthStore::default())),
        auth_required: false,
    })
}

fn api_path(path: &str) -> String {
    format!("{RUNNER_API_BASE_PATH}{path}")
}

async fn build_app_with_bootstrap_auth() -> axum::Router {
    let auth = AuthService::new(Arc::new(InMemoryAuthStore::default()));
    auth.bootstrap_super_admin("admin", Some("admin@example.com"), "password123")
        .await
        .expect("bootstrap super admin should be created");
    build_router(ApiState {
        app: Arc::new(WorkflowApp::new()),
        ai_gateway_base_url: "http://127.0.0.1:6307".to_string(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings: Arc::new(InMemorySystemSettingsStore::new()),
        auth,
        auth_required: false,
    })
}

async fn build_app_with_required_auth() -> axum::Router {
    let auth = AuthService::new(Arc::new(InMemoryAuthStore::default()));
    auth.bootstrap_super_admin("admin", Some("admin@example.com"), "password123")
        .await
        .expect("bootstrap super admin should be created");
    build_router(ApiState {
        app: Arc::new(WorkflowApp::new()),
        ai_gateway_base_url: "http://127.0.0.1:6307".to_string(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings: Arc::new(InMemorySystemSettingsStore::new()),
        auth,
        auth_required: true,
    })
}

async fn login_and_token(app: &axum::Router, login: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "email": login, "password": password }).to_string()))
                .unwrap(),
        )
        .await
        .expect("login request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("login body should be readable")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body).expect("login response should be valid json");
    payload["accessToken"]
        .as_str()
        .expect("login should return access token")
        .to_string()
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

#[tokio::test]
async fn auth_login_and_me_returns_roles_and_permissions() {
    let app = build_app_with_bootstrap_auth().await;
    let token = login_and_token(&app, "admin@example.com", "password123").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("me request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("me body should be readable")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body).expect("me response should be valid json");
    assert_eq!(payload["user"]["username"], json!("admin"));
    assert_eq!(payload["user"]["role"], json!("SUPER_ADMIN"));
    assert!(
        payload["user"]["permissions"]
            .as_array()
            .expect("permissions should be an array")
            .contains(&json!("auth.manage_users"))
    );
}

#[tokio::test]
async fn runner_api_requires_auth_when_enabled_but_keeps_health_public() {
    let app = build_app_with_required_auth().await;
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/workflows"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("workflow request should complete");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let health = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/health"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("health request should complete");
    assert_eq!(health.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_created_station_user_gets_default_wildcard_station_grant() {
    let app = build_app_with_bootstrap_auth().await;
    let admin_token = login_and_token(&app, "admin", "password123").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/admin/users")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {admin_token}"))
                .body(Body::from(
                    json!({
                        "username": "worker-1",
                        "email": "worker@example.com",
                        "password": "123456",
                        "displayName": "Worker One",
                        "roles": ["WORKSTATION_OPERATOR"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("create user request should succeed");
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = create_response
        .into_body()
        .collect()
        .await
        .expect("create user body should be readable")
        .to_bytes();
    let created: Value = serde_json::from_slice(&body).expect("create user response should be valid json");
    let user_id = created["id"].as_str().expect("created user id should exist");

    let station_token = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/station-login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "stationId": "station-1",
                        "platformId": "platform-1",
                        "username": "worker-1",
                        "password": "123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("station login request should succeed");
    assert_eq!(station_token.status(), StatusCode::OK);
    let body = station_token
        .into_body()
        .collect()
        .await
        .expect("station login body should be readable")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body).expect("station login response should be valid json");
    let access_token = payload["accessToken"].as_str().expect("station token should exist");
    assert_eq!(payload["user"]["id"], json!(user_id));

    let authorize = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/station-authorize")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::from(
                    json!({ "requiredPermission": "workstation.operate" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .expect("station authorize request should succeed");
    assert_eq!(authorize.status(), StatusCode::OK);
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
        let _ = sender.send(request);
        stream
            .write_all(response.as_bytes())
            .expect("proxy test server should write response");
    });

    (format!("http://{address}"), receiver)
}

fn spawn_path_response_http_server(responses: Vec<(String, String)>) -> (String, mpsc::Receiver<CapturedHttpRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("path test server should bind to a random port");
    let address = listener
        .local_addr()
        .expect("path test server should expose local address");
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };

            let request = read_http_request(&mut stream);
            let path = request
                .request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            let _ = sender.send(request);
            let response = responses
                .iter()
                .find(|(candidate, _)| candidate == &path)
                .map(|(_, response)| response.as_str())
                .unwrap_or("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream
                .write_all(response.as_bytes())
                .expect("path test server should write response");
        }
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
async fn maps_throttled_errors_to_429() {
    let response =
        crate::modules::ApiError::from(runner::app::AppError::Throttled("too many runs".to_string())).into_response();
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body).expect("error response should be valid json");

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(payload["error"], "too many runs");
}

#[tokio::test]
async fn maps_queue_timeout_errors_to_503() {
    let response = crate::modules::ApiError::from(runner::app::AppError::QueueTimeout("queue timed out".to_string()))
        .into_response();
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body).expect("error response should be valid json");

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(payload["error"], "queue timed out");
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
async fn registers_http_plugin_and_lists_node_descriptors() {
    let descriptor_body = json!([
        {
            "id": "barcode_scan",
            "kind": "effect",
            "runnerType": "plugin:barcode_scan",
            "version": "1.0.0",
            "category": "业务节点",
            "displayName": "条码扫描",
            "color": "#0EA5E9",
            "icon": "scan-line",
            "transport": "http",
            "configSchema": {
                "type": "object"
            },
            "supportsCancel": true,
            "supportsResume": true
        },
        {
            "id": "barcode_bind",
            "kind": "effect",
            "runnerType": "plugin:barcode_bind",
            "version": "1.0.0",
            "category": "业务节点",
            "displayName": "条码绑定",
            "transport": "http",
            "configSchema": {
                "type": "object"
            },
            "supportsCancel": false,
            "supportsResume": false
        }
    ])
    .to_string();
    let upstream_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        descriptor_body.len(),
        descriptor_body
    );
    let (plugin_base_url, captured_request_receiver) = spawn_single_response_http_server(upstream_response);
    let app = build_app();

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path("/plugin-registrations"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "baseUrl": plugin_base_url
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("plugin registration request should succeed");

    assert_eq!(register_response.status(), StatusCode::CREATED);
    let captured = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("plugin descriptor request should be captured");
    assert!(
        captured.request_line.starts_with("GET /descriptors ") || captured.request_line.starts_with("GET /descriptor ")
    );

    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/node-descriptors"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("descriptor list request should succeed");
    assert_eq!(list_response.status(), StatusCode::OK);

    let payload: Value = serde_json::from_slice(
        &list_response
            .into_body()
            .collect()
            .await
            .expect("response body should collect")
            .to_bytes(),
    )
    .expect("descriptor list should be valid json");
    let items = payload.as_array().expect("descriptor list should be an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["runnerType"], json!("plugin:barcode_bind"));
    assert_eq!(items[1]["runnerType"], json!("plugin:barcode_scan"));
    assert_eq!(items[0]["endpoint"], json!(plugin_base_url));
    assert_eq!(items[1]["endpoint"], json!(plugin_base_url));
    assert_eq!(items[1]["color"], json!("#0EA5E9"));
    assert_eq!(items[1]["icon"], json!("scan-line"));
}

#[tokio::test]
async fn auto_registers_http_plugins_from_base_urls() {
    let descriptor_body = json!([
        {
            "id": "hello_world",
            "kind": "effect",
            "runnerType": "plugin:hello_world",
            "version": "1.0.0",
            "category": "业务节点",
            "displayName": "Hello World",
            "transport": "http",
            "configSchema": {
                "type": "object"
            },
            "supportsCancel": false,
            "supportsResume": false
        },
        {
            "id": "hello_world_formal",
            "kind": "effect",
            "runnerType": "plugin:hello_world_formal",
            "version": "1.0.0",
            "category": "业务节点",
            "displayName": "Hello World Formal",
            "transport": "http",
            "configSchema": {
                "type": "object"
            },
            "supportsCancel": false,
            "supportsResume": false
        }
    ])
    .to_string();
    let upstream_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        descriptor_body.len(),
        descriptor_body
    );
    let (plugin_base_url, captured_request_receiver) = spawn_single_response_http_server(upstream_response);
    let state = ApiState {
        app: Arc::new(WorkflowApp::new()),
        ai_gateway_base_url: "http://127.0.0.1:6307".to_string(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings: Arc::new(InMemorySystemSettingsStore::new()),
        auth: AuthService::new(Arc::new(InMemoryAuthStore::default())),
        auth_required: false,
    };

    let descriptors = register_http_plugin_base_urls(&state, std::slice::from_ref(&plugin_base_url))
        .await
        .expect("auto registration should succeed");

    assert_eq!(descriptors.len(), 2);
    assert_eq!(descriptors[0].endpoint.as_deref(), Some(plugin_base_url.as_str()));
    assert_eq!(descriptors[1].endpoint.as_deref(), Some(plugin_base_url.as_str()));

    let captured = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("plugin descriptor request should be captured");
    assert!(
        captured.request_line.starts_with("GET /descriptors ") || captured.request_line.starts_with("GET /descriptor ")
    );

    let registered = state
        .app
        .list_node_descriptors()
        .expect("descriptor list should be available");
    assert_eq!(registered.len(), 2);
    assert_eq!(registered[0].runner_type, "plugin:hello_world");
    assert_eq!(registered[1].runner_type, "plugin:hello_world_formal");
}

#[tokio::test]
async fn falls_back_to_legacy_single_descriptor_endpoint() {
    let descriptor_body = json!({
        "id": "legacy_plugin",
        "kind": "effect",
        "runnerType": "plugin:legacy_plugin",
        "version": "1.0.0",
        "category": "业务节点",
        "displayName": "Legacy Plugin",
        "transport": "http",
        "configSchema": {
            "type": "object"
        },
        "supportsCancel": false,
        "supportsResume": false
    })
    .to_string();
    let descriptor_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        descriptor_body.len(),
        descriptor_body
    );
    let (plugin_base_url, captured_request_receiver) =
        spawn_path_response_http_server(vec![("/descriptor".to_string(), descriptor_response)]);
    let state = ApiState {
        app: Arc::new(WorkflowApp::new()),
        ai_gateway_base_url: "http://127.0.0.1:6307".to_string(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings: Arc::new(InMemorySystemSettingsStore::new()),
        auth: AuthService::new(Arc::new(InMemoryAuthStore::default())),
        auth_required: false,
    };

    let descriptors = register_http_plugin_base_urls(&state, std::slice::from_ref(&plugin_base_url))
        .await
        .expect("legacy auto registration should succeed");

    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].runner_type, "plugin:legacy_plugin");

    let first = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("descriptors request should be captured");
    let second = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("descriptor fallback request should be captured");
    assert!(first.request_line.starts_with("GET /descriptors "));
    assert!(second.request_line.starts_with("GET /descriptor "));
}

#[tokio::test]
async fn updates_plugin_auto_registration_config_and_registers_plugins() {
    let descriptor_body = json!([
        {
            "id": "auto_plugin",
            "kind": "effect",
            "runnerType": "plugin:auto_plugin",
            "version": "1.0.0",
            "category": "业务节点",
            "displayName": "Auto Plugin",
            "transport": "http",
            "configSchema": {
                "type": "object"
            },
            "supportsCancel": false,
            "supportsResume": false
        }
    ])
    .to_string();
    let upstream_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        descriptor_body.len(),
        descriptor_body
    );
    let (plugin_base_url, captured_request_receiver) = spawn_single_response_http_server(upstream_response);
    let app = build_app();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(api_path("/system/plugin-auto-registration"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "baseUrls": [plugin_base_url]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("system config update request should succeed");

    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload: Value = serde_json::from_slice(
        &update_response
            .into_body()
            .collect()
            .await
            .expect("response body should collect")
            .to_bytes(),
    )
    .expect("system config update response should be valid json");
    assert_eq!(update_payload["baseUrls"], json!([plugin_base_url]));
    assert_eq!(
        update_payload["descriptors"][0]["runnerType"],
        json!("plugin:auto_plugin")
    );

    let captured = captured_request_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("plugin descriptor request should be captured");
    assert!(
        captured.request_line.starts_with("GET /descriptors ") || captured.request_line.starts_with("GET /descriptor ")
    );

    let get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/system/plugin-auto-registration"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("system config get request should succeed");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: Value = serde_json::from_slice(
        &get_response
            .into_body()
            .collect()
            .await
            .expect("response body should collect")
            .to_bytes(),
    )
    .expect("system config get response should be valid json");
    assert_eq!(get_payload["baseUrls"], update_payload["baseUrls"]);
}

#[tokio::test]
async fn replacing_plugin_auto_registration_config_removes_stale_descriptors() {
    let first_descriptor_body = json!([{
        "id": "plugin_a",
        "kind": "effect",
        "runnerType": "plugin:plugin_a",
        "version": "1.0.0",
        "category": "业务节点",
        "displayName": "Plugin A",
        "transport": "http",
        "configSchema": {
            "type": "object"
        },
        "supportsCancel": false,
        "supportsResume": false
    }])
    .to_string();
    let second_descriptor_body = json!([{
        "id": "plugin_b",
        "kind": "effect",
        "runnerType": "plugin:plugin_b",
        "version": "1.0.0",
        "category": "业务节点",
        "displayName": "Plugin B",
        "transport": "http",
        "configSchema": {
            "type": "object"
        },
        "supportsCancel": false,
        "supportsResume": false
    }])
    .to_string();
    let first_upstream_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        first_descriptor_body.len(),
        first_descriptor_body
    );
    let second_upstream_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        second_descriptor_body.len(),
        second_descriptor_body
    );
    let (first_plugin_base_url, _) = spawn_single_response_http_server(first_upstream_response);
    let (second_plugin_base_url, _) = spawn_single_response_http_server(second_upstream_response);
    let app = build_app();

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(api_path("/system/plugin-auto-registration"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "baseUrls": [first_plugin_base_url]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("first system config update should succeed");
    assert_eq!(first_response.status(), StatusCode::OK);

    let second_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(api_path("/system/plugin-auto-registration"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "baseUrls": [second_plugin_base_url]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("second system config update should succeed");
    assert_eq!(second_response.status(), StatusCode::OK);

    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/node-descriptors"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("descriptor list request should succeed");
    assert_eq!(list_response.status(), StatusCode::OK);

    let payload: Value = serde_json::from_slice(
        &list_response
            .into_body()
            .collect()
            .await
            .expect("response body should collect")
            .to_bytes(),
    )
    .expect("descriptor list should be valid json");
    let items = payload.as_array().expect("descriptor list should be an array");

    assert!(
        items.iter().any(|item| item["runnerType"] == json!("plugin:plugin_b")),
        "newly configured plugin should remain registered"
    );
    assert!(
        items.iter().all(|item| item["runnerType"] != json!("plugin:plugin_a")),
        "stale plugin descriptor should be removed"
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
async fn run_endpoint_reuses_active_workflow_for_unique_key() {
    let app = build_app();
    let workflow = json!({
        "meta": {
            "key": "api-idempotent-flow",
            "name": "API Idempotent Flow",
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

    let first_run_id = start_run_with_unique_key(
        app.clone(),
        &workflow_id,
        "order:SO-API-IDEMPOTENT-1",
        json!({
            "body": {
                "orderNo": "SO-API-IDEMPOTENT-1"
            }
        }),
    )
    .await;
    let waiting = wait_for_status(app.clone(), &first_run_id, "waiting").await;
    assert_eq!(waiting["status"], json!("waiting"));

    let second_run_id = start_run_with_unique_key(
        app,
        &workflow_id,
        "order:SO-API-IDEMPOTENT-1",
        json!({
            "body": {
                "orderNo": "SO-API-IDEMPOTENT-1"
            }
        }),
    )
    .await;

    assert_eq!(second_run_id, first_run_id);
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
        .clone()
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

    let lightweight_get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path(&format!(
                    "/edit-sessions/{session_id}?includeEditorDocument=false"
                )))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should succeed");

    assert_eq!(lightweight_get_response.status(), StatusCode::OK);
    let lightweight_get_body = lightweight_get_response
        .into_body()
        .collect()
        .await
        .expect("body should collect")
        .to_bytes();
    let lightweight_get_payload: Value =
        serde_json::from_slice(&lightweight_get_body).expect("response body should be valid json");

    assert!(lightweight_get_payload.get("editorDocument").is_none());
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
async fn patches_edit_session_draft_with_batch_node_config_and_edge_operations() {
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
            { "id": "fetch_1", "type": "fetch", "name": "Fetch", "config": { "url": "https://old.example.com" } },
            { "id": "wait_1", "type": "wait", "name": "Wait" },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "fetch_1" }
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
                            "graph": {
                                "nodes": [
                                    { "id": "start_1", "type": "terminal" },
                                    { "id": "fetch_1", "type": "workflow-card" },
                                    { "id": "wait_1", "type": "workflow-card" },
                                    { "id": "end_1", "type": "terminal" }
                                ],
                                "edges": [
                                    {
                                        "id": "edge:start_1:out->fetch_1:in",
                                        "source": "start_1",
                                        "sourceHandle": "out",
                                        "target": "fetch_1",
                                        "targetHandle": "in"
                                    }
                                ],
                                "panels": {
                                    "start_1": { "tabs": ["base"], "fieldsByTab": {} },
                                    "fetch_1": { "tabs": ["base"], "fieldsByTab": {} },
                                    "wait_1": { "tabs": ["base"], "fieldsByTab": {} },
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
                                "type": "update_node_config",
                                "nodeId": "fetch_1",
                                "config": {
                                    "url": "https://api.example.com",
                                    "method": "POST"
                                }
                            },
                            {
                                "type": "add_edge",
                                "source": "fetch_1",
                                "target": "wait_1",
                                "sourceHandle": "out",
                                "targetHandle": "in"
                            },
                            {
                                "type": "update_edge",
                                "edgeId": "edge:fetch_1:out->wait_1:in",
                                "updates": {
                                    "target": "end_1",
                                    "targetHandle": "in",
                                    "label": "success",
                                    "priority": 5
                                }
                            },
                            {
                                "type": "remove_edge",
                                "edgeId": "edge:fetch_1:out->end_1:in"
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
            .find(|node| node["id"].as_str() == Some("fetch_1"))
            .expect("fetch node should exist")["config"],
        json!({
            "url": "https://api.example.com",
            "method": "POST"
        })
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
        vec![("start_1", "fetch_1")]
    );
    assert!(
        !patch_payload["editorDocument"]["graph"]["edges"]
            .as_array()
            .expect("editor document edges should exist")
            .iter()
            .any(|edge| edge["id"].as_str() == Some("edge:fetch_1:out->end_1:in"))
    );
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

#[tokio::test]
async fn searches_runs_by_request_id_and_order_no() {
    let app = build_app();
    let workflow_id = upload_workflow(
        app.clone(),
        json!({
            "meta": {
                "key": "searchable-flow",
                "name": "Searchable Flow",
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
        }),
    )
    .await;

    let run_id = start_run(
        app.clone(),
        &workflow_id,
        json!({
            "headers": {
                "requestId": "req-search-1"
            },
            "body": {
                "orderNo": "SO-SEARCH-1",
                "waveNo": "WAVE-SEARCH-1"
            }
        }),
    )
    .await;

    let _summary = wait_for_terminal_status(app.clone(), &run_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(api_path("/runs/search?requestId=req-search-1&orderNo=SO-SEARCH-1"))
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

    assert_eq!(payload["total"], json!(1));
    assert_eq!(payload["items"][0]["runId"], json!(run_id));
    assert_eq!(payload["items"][0]["requestId"], json!("req-search-1"));
    assert_eq!(payload["items"][0]["orderNo"], json!("SO-SEARCH-1"));
    assert_eq!(payload["items"][0]["waveNo"], json!("WAVE-SEARCH-1"));
}

#[tokio::test]
async fn appends_manual_patch_note_to_run_timeline() {
    let app = build_app();
    let workflow_id = upload_workflow(
        app.clone(),
        json!({
            "meta": {
                "key": "manual-patch-flow",
                "name": "Manual Patch Flow",
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
        }),
    )
    .await;

    let run_id = start_run(
        app.clone(),
        &workflow_id,
        json!({
            "body": {
                "orderNo": "SO-MANUAL-1"
            }
        }),
    )
    .await;

    let _summary = wait_for_terminal_status(app.clone(), &run_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/runs/{run_id}/manual-patch")))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "nodeId": "end_1",
                        "note": "人工确认已处理",
                        "operator": "张工"
                    }))
                    .expect("request should serialize"),
                ))
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

    assert!(
        payload["timeline"]
            .as_array()
            .is_some_and(|timeline| timeline.iter().any(|entry| {
                entry["nodeId"] == json!("end_1")
                    && entry["logs"].as_array().is_some_and(|logs| {
                        logs.iter().any(|log| {
                            log["level"] == json!("manual")
                                && log["message"]
                                    .as_str()
                                    .is_some_and(|message| message.contains("人工确认已处理"))
                        })
                    })
            })),
        "manual patch note should be appended to the node logs",
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

async fn start_run_with_unique_key(app: axum::Router, workflow_id: &str, unique_key: &str, trigger: Value) -> String {
    let execute_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(api_path(&format!("/workflows/{workflow_id}/run")))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "uniqueKey": unique_key,
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
