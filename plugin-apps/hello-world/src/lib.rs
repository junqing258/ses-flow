use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PLUGIN_ID: &str = "hello_world";
pub const PLUGIN_RUNNER_TYPE: &str = "plugin:hello_world";

pub fn build_app() -> Router {
    Router::new()
        .route("/descriptor", get(get_descriptor))
        .route("/health", get(get_health))
        .route("/execute", post(execute))
        .route("/cancel", post(cancel))
        .route("/resume", post(resume))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub id: String,
    pub kind: String,
    pub runner_type: String,
    pub version: String,
    pub category: String,
    pub display_name: String,
    pub description: String,
    pub status: String,
    pub transport: String,
    pub timeout_ms: u64,
    pub supports_cancel: bool,
    pub supports_resume: bool,
    pub config_schema: Value,
    pub defaults: Value,
    pub input_mapping_schema: Value,
    pub output_mapping_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub plugin_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteRequest {
    pub node_id: String,
    #[serde(default)]
    pub config: Value,
    pub context: ExecuteContext,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteContext {
    pub run_id: String,
    pub request_id: String,
    #[serde(default)]
    pub trace_id: Option<String>,
    pub workflow_key: String,
    pub workflow_version: u32,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub state: Value,
    #[serde(default)]
    pub env: Value,
    #[serde(default)]
    pub resume_signal: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub run_id: String,
    pub request_id: String,
    pub node_id: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeRequest {
    pub run_id: String,
    pub request_id: String,
    pub node_id: String,
    #[serde(default)]
    pub signal: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResponse {
    pub status: String,
    pub output: Value,
    pub state_patch: Value,
    pub logs: Vec<PluginLogRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLogRecord {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
}

async fn get_descriptor() -> Json<PluginDescriptor> {
    Json(plugin_descriptor())
}

async fn get_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        plugin_id: PLUGIN_ID.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn execute(Json(request): Json<ExecuteRequest>) -> Response {
    let target = request_target(&request);
    let prefix = request_prefix(&request);
    let message = format!("{prefix}, {target}!");
    let ExecuteRequest {
        node_id,
        config,
        context,
    } = request;
    let ExecuteContext {
        run_id,
        request_id,
        trace_id,
        workflow_key,
        workflow_version,
        input,
        ..
    } = context;
    let input_echo = input.clone();
    let trace_id_header = trace_id.clone();
    let node_id_for_output = node_id.clone();
    let node_id_for_state = node_id.clone();
    let node_id_for_log = node_id.clone();
    let run_id_for_output = run_id.clone();
    let run_id_for_state = run_id.clone();
    let request_id_for_output = request_id.clone();
    let request_id_for_state = request_id.clone();
    let workflow_key_for_output = workflow_key.clone();
    let workflow_key_for_log = workflow_key.clone();
    let trace_id_for_output = trace_id.clone();
    let trace_id_for_state = trace_id.clone();

    let response = ExecuteResponse {
        status: "success".to_string(),
        output: json!({
            "message": message,
            "pluginId": PLUGIN_ID,
            "nodeId": node_id_for_output,
            "runId": run_id_for_output,
            "requestId": request_id_for_output,
            "traceId": trace_id_for_output,
            "workflowKey": workflow_key_for_output,
            "workflowVersion": workflow_version,
            "receivedInput": input,
            "receivedConfig": config
        }),
        state_patch: json!({
            "plugins": {
                PLUGIN_ID: {
                    "lastGreeting": message,
                    "lastRunId": run_id_for_state,
                    "lastRequestId": request_id_for_state,
                    "lastNodeId": node_id_for_state,
                    "traceId": trace_id_for_state,
                    "inputEcho": input_echo
                }
            }
        }),
        logs: vec![PluginLogRecord {
            level: "info".to_string(),
            message: format!("hello-world executed for {target}"),
            fields: json!({
                "pluginId": PLUGIN_ID,
                "nodeId": node_id_for_log,
                "workflowKey": workflow_key_for_log
            }),
        }],
    };

    json_response(StatusCode::OK, &response, trace_id_header.as_deref())
}

async fn cancel(Json(request): Json<CancelRequest>) -> Response {
    let message = format!(
        "plugin {PLUGIN_ID} does not implement cancel for node {}",
        request.node_id
    );
    let response = ErrorResponse { error: message };
    json_response(StatusCode::NOT_IMPLEMENTED, &response, None)
}

async fn resume(Json(request): Json<ResumeRequest>) -> Response {
    let message = format!(
        "plugin {PLUGIN_ID} does not implement resume for node {}",
        request.node_id
    );
    let response = ErrorResponse { error: message };
    json_response(StatusCode::NOT_IMPLEMENTED, &response, None)
}

pub fn plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: PLUGIN_ID.to_string(),
        kind: "effect".to_string(),
        runner_type: PLUGIN_RUNNER_TYPE.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        category: "业务节点".to_string(),
        display_name: "Hello World".to_string(),
        description: "示例 HTTP 插件节点，返回一条问候消息并回写执行结果。".to_string(),
        status: "stable".to_string(),
        transport: "http".to_string(),
        timeout_ms: 5_000,
        supports_cancel: false,
        supports_resume: false,
        config_schema: json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "title": "默认问候对象",
                    "x-tab": "base",
                    "x-component": "input"
                },
                "prefix": {
                    "type": "string",
                    "title": "问候前缀",
                    "x-tab": "base",
                    "x-component": "input"
                }
            }
        }),
        defaults: json!({
            "target": "World",
            "prefix": "Hello"
        }),
        input_mapping_schema: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "title": "运行时问候对象"
                }
            }
        }),
        output_mapping_schema: json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "title": "问候消息"
                },
                "runId": {
                    "type": "string",
                    "title": "运行实例 ID"
                },
                "requestId": {
                    "type": "string",
                    "title": "请求 ID"
                }
            }
        }),
    }
}

fn request_target(request: &ExecuteRequest) -> String {
    extract_string(&request.context.input, &["name", "target"])
        .or_else(|| extract_string(&request.config, &["target", "name"]))
        .unwrap_or_else(|| "World".to_string())
}

fn request_prefix(request: &ExecuteRequest) -> String {
    extract_string(&request.config, &["prefix"]).unwrap_or_else(|| "Hello".to_string())
}

fn extract_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(key))
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn json_response<T>(status: StatusCode, payload: &T, trace_id: Option<&str>) -> Response
where
    T: Serialize,
{
    let mut response = (status, Json(payload)).into_response();
    if let Some(trace_id) = trace_id {
        if let Ok(value) = HeaderValue::from_str(trace_id) {
            response.headers_mut().insert("X-Trace-Id", value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

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
    }

    #[tokio::test]
    async fn execute_route_returns_success_payload_and_trace_header() {
        let payload = json!({
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
}
