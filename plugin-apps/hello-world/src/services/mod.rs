use serde_json::{Value, json};

use crate::models::{
    ErrorResponse, ExecuteContext, ExecuteRequest, ExecuteResponse, FORMAL_PLUGIN_ID, FORMAL_PLUGIN_RUNNER_TYPE,
    FORMAL_PLUGIN_RUNNER_TYPE as FORMAL_TYPE, HealthResponse, PLUGIN_ID, PLUGIN_RUNNER_TYPE, PluginDescriptor,
    PluginLogRecord,
};

pub(crate) fn health_response() -> HealthResponse {
    HealthResponse {
        status: "ok".to_string(),
        plugin_id: PLUGIN_ID.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub(crate) fn plugin_descriptor() -> PluginDescriptor {
    plugin_descriptors()
        .into_iter()
        .next()
        .expect("hello-world plugin should expose at least one descriptor")
}

pub(crate) fn plugin_descriptors() -> Vec<PluginDescriptor> {
    vec![create_hello_world_descriptor(), create_formal_hello_world_descriptor()]
}

pub(crate) fn execute_response(request: ExecuteRequest) -> (ExecuteResponse, Option<String>) {
    let target = request_target(&request);
    let prefix = request_prefix(&request);
    let message = format_message(&request, &prefix, &target);
    let ExecuteRequest {
        plugin_id,
        runner_type,
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

    (
        ExecuteResponse {
            status: "success".to_string(),
            output: json!({
                "message": message,
                "pluginId": plugin_id,
                "runnerType": runner_type,
                "nodeId": node_id,
                "runId": run_id,
                "requestId": request_id,
                "traceId": trace_id,
                "workflowKey": workflow_key,
                "workflowVersion": workflow_version,
                "receivedInput": input,
                "receivedConfig": config
            }),
            state_patch: json!({
                "plugins": {
                    plugin_id.clone(): {
                        "lastGreeting": message,
                        "lastRunId": run_id,
                        "lastRequestId": request_id,
                        "lastNodeId": node_id,
                        "traceId": trace_id,
                        "inputEcho": input_echo
                    }
                }
            }),
            logs: vec![PluginLogRecord {
                level: "info".to_string(),
                message: format!("hello-world executed for {target}"),
                fields: json!({
                    "pluginId": plugin_id,
                    "runnerType": runner_type,
                    "nodeId": node_id,
                    "workflowKey": workflow_key
                }),
            }],
        },
        trace_id_header,
    )
}

pub(crate) fn cancel_response(node_id: &str) -> ErrorResponse {
    ErrorResponse {
        error: format!("plugin {PLUGIN_ID} does not implement cancel for node {node_id}"),
    }
}

pub(crate) fn resume_response(node_id: &str) -> ErrorResponse {
    ErrorResponse {
        error: format!("plugin {PLUGIN_ID} does not implement resume for node {node_id}"),
    }
}

fn create_hello_world_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: PLUGIN_ID.to_string(),
        kind: "effect".to_string(),
        runner_type: PLUGIN_RUNNER_TYPE.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        category: "业务节点".to_string(),
        display_name: "Hello World".to_string(),
        description: "示例 HTTP 插件节点，返回一条问候消息并回写执行结果。".to_string(),
        color: Some("#0EA5E9".to_string()),
        icon: Some("sparkles".to_string()),
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

fn create_formal_hello_world_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: FORMAL_PLUGIN_ID.to_string(),
        kind: "effect".to_string(),
        runner_type: FORMAL_PLUGIN_RUNNER_TYPE.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        category: "业务节点".to_string(),
        display_name: "Hello World Formal".to_string(),
        description: "示例 HTTP 插件节点，返回更正式的问候消息。".to_string(),
        color: Some("#7C3AED".to_string()),
        icon: Some("badge-check".to_string()),
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
                    "title": "正式问候前缀",
                    "x-tab": "base",
                    "x-component": "input"
                }
            }
        }),
        defaults: json!({
            "target": "World",
            "prefix": "Greetings"
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
                    "title": "正式问候消息"
                },
                "runnerType": {
                    "type": "string",
                    "title": "节点类型"
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
    extract_string(&request.config, &["prefix"])
        .unwrap_or_else(|| default_prefix_for_runner_type(&request.runner_type).to_string())
}

fn default_prefix_for_runner_type(runner_type: &str) -> &'static str {
    match runner_type {
        FORMAL_TYPE => "Greetings",
        _ => "Hello",
    }
}

fn format_message(request: &ExecuteRequest, prefix: &str, target: &str) -> String {
    if request.runner_type == FORMAL_PLUGIN_RUNNER_TYPE {
        format!("{prefix}, {target}.")
    } else {
        format!("{prefix}, {target}!")
    }
}

fn extract_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(key))
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
