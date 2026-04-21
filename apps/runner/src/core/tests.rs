use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::json;

use super::definition::{NodeType, WorkflowDefinition, deserialize_workflow_definition};
use super::engine::WorkflowEngine;
use super::runtime::{RunEnvironment, WorkflowRunObserver, WorkflowRunStatus, WorkflowRunSummary};

#[derive(Default)]
struct RecordingObserver {
    summaries: Mutex<Vec<WorkflowRunSummary>>,
}

impl RecordingObserver {
    fn snapshot(&self) -> Vec<WorkflowRunSummary> {
        self.summaries
            .lock()
            .expect("observer summaries lock should not be poisoned")
            .clone()
    }
}

impl WorkflowRunObserver for RecordingObserver {
    fn on_summary(&self, summary: &WorkflowRunSummary) {
        self.summaries
            .lock()
            .expect("observer summaries lock should not be poisoned")
            .push(summary.clone());
    }
}

fn node_supports_typescript_code() -> bool {
    let Ok(output) = Command::new("node").arg("-v").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let trimmed = version.trim().trim_start_matches('v');
    let mut parts = trimmed.split('.');
    let major = parts.next().and_then(|value| value.parse::<u32>().ok());
    let minor = parts.next().and_then(|value| value.parse::<u32>().ok());

    matches!((major, minor), (Some(major), Some(minor)) if major > 22 || (major == 22 && minor >= 20))
}

fn spawn_echo_http_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("echo test server should bind to a random port");
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
            let mut header_end = None;
            let mut content_length = 0usize;

            loop {
                let read = stream.read(&mut chunk).expect("echo test server should read request");
                if read == 0 {
                    break;
                }

                buffer.extend_from_slice(&chunk[..read]);

                if header_end.is_none() {
                    header_end = buffer
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .map(|index| index + 4);

                    if let Some(end) = header_end {
                        let header_text = String::from_utf8_lossy(&buffer[..end]).to_string();
                        content_length = parse_content_length(&header_text);
                    }
                }

                if let Some(end) = header_end {
                    if buffer.len() >= end + content_length {
                        break;
                    }
                }
            }

            let request_text = String::from_utf8_lossy(&buffer).to_string();
            let response_body = build_echo_response(&request_text);
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

fn parse_content_length(header_text: &str) -> usize {
    header_text
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

fn build_echo_response(raw_request: &str) -> String {
    let (raw_headers, raw_body) = raw_request.split_once("\r\n\r\n").unwrap_or((raw_request, ""));
    let mut lines = raw_headers.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut request_line_parts = request_line.split_whitespace();
    let method = request_line_parts.next().unwrap_or_default().to_string();
    let target = request_line_parts.next().unwrap_or("/").to_string();
    let (path, query) = split_target(&target);
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<HashMap<_, _>>();
    let body = if raw_body.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(raw_body).unwrap_or_else(|_| serde_json::Value::String(raw_body.to_string()))
    };

    json!({
        "status": "loaded",
        "method": method,
        "path": path,
        "query": query,
        "body": body,
        "headers": headers,
        "orderNo": query.get("orderNo").cloned().unwrap_or_else(|| body.get("orderNo").cloned().unwrap_or(serde_json::Value::Null)),
        "warehouseId": query.get("warehouseId").cloned().unwrap_or_else(|| body.get("warehouseId").cloned().unwrap_or(serde_json::Value::Null))
    })
    .to_string()
}

fn split_target(target: &str) -> (String, serde_json::Value) {
    let Some((path, query_string)) = target.split_once('?') else {
        return (target.to_string(), json!({}));
    };

    let query = query_string
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((key.to_string(), serde_json::Value::String(value.to_string())))
        })
        .collect::<serde_json::Map<_, _>>();

    (path.to_string(), serde_json::Value::Object(query))
}

fn load_sorting_flow_definition(fetch_base_url: &str) -> WorkflowDefinition {
    let mut definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
    let fetch_node = definition
        .nodes
        .iter_mut()
        .find(|node| node.id == "fetch_order")
        .expect("sorting flow should contain fetch node");
    fetch_node.config = json!({
        "method": "GET",
        "url": format!("{fetch_base_url}/todos")
    });
    definition
}

fn load_coverage_flow_definition(fetch_base_url: &str) -> WorkflowDefinition {
    let mut definition: WorkflowDefinition = serde_json::from_str(include_str!("../../examples/coverage-flow.json"))
        .expect("coverage workflow should deserialize");
    let fetch_node = definition
        .nodes
        .iter_mut()
        .find(|node| node.id == "fetch_context")
        .expect("coverage flow should contain fetch node");
    fetch_node.config = json!({
        "method": "GET",
        "url": format!("{fetch_base_url}/todos")
    });
    definition
}

#[test]
fn upgrades_legacy_action_node_type_during_definition_deserialization() {
    let definition = deserialize_workflow_definition(json!({
        "meta": {
            "key": "legacy-action-upgrade",
            "name": "Legacy Action Upgrade",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "legacy_node",
                "type": "action",
                "name": "Legacy Node",
                "config": {
                    "command": "printf '{}'"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "legacy_node" },
            { "from": "legacy_node", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("legacy action definition should deserialize");

    assert_eq!(
        definition.node("legacy_node").map(|node| node.node_type),
        Some(NodeType::Shell)
    );
}

#[test]
fn upgrades_legacy_action_nodes_in_nested_sub_workflow_definitions() {
    let definition = deserialize_workflow_definition(json!({
        "meta": {
            "key": "legacy-subflow-upgrade",
            "name": "Legacy Subflow Upgrade",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "subflow_1",
                "type": "sub_workflow",
                "name": "Subflow",
                "config": {
                    "definition": {
                        "meta": {
                            "key": "legacy-child",
                            "name": "Legacy Child",
                            "version": 1
                        },
                        "trigger": {
                            "type": "manual"
                        },
                        "inputSchema": { "type": "object" },
                        "nodes": [
                            { "id": "child_start", "type": "start", "name": "Start" },
                            {
                                "id": "child_action",
                                "type": "action",
                                "name": "Legacy Child Action",
                                "config": {
                                    "command": "printf '{}'"
                                }
                            },
                            { "id": "child_end", "type": "end", "name": "End" }
                        ],
                        "transitions": [
                            { "from": "child_start", "to": "child_action" },
                            { "from": "child_action", "to": "child_end" }
                        ],
                        "policies": {}
                    }
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "subflow_1" },
            { "from": "subflow_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("legacy nested definition should deserialize");

    let subflow_node = definition.node("subflow_1").expect("subflow node should exist");
    let nested = subflow_node
        .config
        .get("definition")
        .cloned()
        .expect("nested definition should exist");
    let nested_definition =
        deserialize_workflow_definition(nested).expect("nested definition should deserialize after normalization");

    assert_eq!(
        nested_definition.node("child_action").map(|node| node.node_type),
        Some(NodeType::Shell)
    );
}

#[test]
fn waits_on_task_branch_for_manual_review() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-1" },
                "body": { "orderNo": "SO-1001", "bizType": "manual_review" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Waiting));
    assert_eq!(summary.current_node_id.as_deref(), Some("manual_review_task"));
}

#[test]
fn waits_on_callback_branch_for_auto_sort() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-2" },
                "body": { "orderNo": "SO-1002", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Waiting));
    assert_eq!(summary.current_node_id.as_deref(), Some("wait_dispatch_callback"));
    assert_eq!(summary.state["orderSnapshot"]["data"]["orderNo"], json!("SO-1002"));
}

#[test]
fn resumes_waiting_callback_to_completion() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let waiting_summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-3" },
                "body": { "orderNo": "SO-1003", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    let resumed = engine
        .resume(
            &definition,
            waiting_summary
                .resume_state
                .expect("waiting run should expose resume state"),
            json!({
                "event": "rcs.callback",
                "correlationKey": "req-3",
                "status": "done",
                "orderNo": "SO-1003"
            }),
        )
        .expect("resume should complete");

    assert!(matches!(resumed.status, WorkflowRunStatus::Completed));
    assert_eq!(resumed.current_node_id.as_deref(), Some("end_1"));
    assert_eq!(
        resumed
            .timeline
            .iter()
            .rev()
            .find(|record| record.node_id == "wait_dispatch_callback")
            .expect("resumed wait node should exist in timeline")
            .status,
        crate::core::runtime::ExecutionStatus::Success
    );
    assert_eq!(
        resumed.timeline.last().expect("timeline should not be empty").output,
        json!({
            "correlationKey": "req-3",
            "event": "rcs.callback",
            "status": "done",
            "orderNo": "SO-1003"
        })
    );
}

#[test]
fn fetch_node_supports_http_get_query_requests() {
    let server_url = spawn_echo_http_server();
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": {
            "key": "http-fetch-get",
            "name": "HTTP Fetch GET",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "fetch_todo",
                "type": "fetch",
                "name": "Fetch Todo",
                "config": {
                    "method": "GET",
                    "url": format!("{server_url}/todos"),
                    "headers": {
                        "x-source": "runner-test"
                    }
                },
                "inputMapping": {
                    "orderNo": "{{trigger.body.orderNo}}",
                    "warehouseId": "{{env.warehouseId}}"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "fetch_todo" },
            { "from": "fetch_todo", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("fetch workflow should deserialize");
    let engine = WorkflowEngine::new();

    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-HTTP-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("http GET fetch should succeed");

    let output = &summary
        .timeline
        .iter()
        .find(|record| record.node_id == "fetch_todo")
        .expect("fetch timeline item should exist")
        .output;
    assert_eq!(output["method"], json!("GET"));
    assert_eq!(output["response"]["status"], json!(200));
    assert_eq!(output["data"]["path"], json!("/todos"));
    assert_eq!(output["data"]["query"]["orderNo"], json!("SO-HTTP-1"));
    assert_eq!(output["data"]["query"]["warehouseId"], json!("WH-1"));
}

#[test]
fn fetch_node_supports_http_post_json_requests() {
    let server_url = spawn_echo_http_server();
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": {
            "key": "http-fetch-post",
            "name": "HTTP Fetch POST",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "post_todo",
                "type": "fetch",
                "name": "Create Todo",
                "config": {
                    "method": "POST",
                    "url": format!("{server_url}/todos"),
                    "headers": {
                        "content-type": "application/json"
                    }
                },
                "inputMapping": {
                    "orderNo": "{{trigger.body.orderNo}}",
                    "title": "new todo"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "post_todo" },
            { "from": "post_todo", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("post workflow should deserialize");
    let engine = WorkflowEngine::new();

    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-HTTP-POST-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("http POST fetch should succeed");

    let output = &summary
        .timeline
        .iter()
        .find(|record| record.node_id == "post_todo")
        .expect("fetch timeline item should exist")
        .output;
    assert_eq!(output["method"], json!("POST"));
    assert_eq!(output["response"]["ok"], json!(true));
    assert_eq!(output["data"]["body"]["orderNo"], json!("SO-HTTP-POST-1"));
    assert_eq!(output["data"]["body"]["title"], json!("new todo"));
}

#[test]
fn shell_node_executes_command_and_parses_json_stdout() {
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": {
            "key": "shell-node-demo",
            "name": "Shell Node Demo",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "shell_1",
                "type": "shell",
                "name": "Run Shell",
                "config": {
                    "command": "printf '%s' \"$WORKFLOW_PARAMS\"",
                    "shell": "sh"
                },
                "inputMapping": {
                    "orderNo": "{{trigger.body.orderNo}}",
                    "tenantId": "{{env.tenantId}}"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "shell_1" },
            { "from": "shell_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("shell workflow should deserialize");
    let engine = WorkflowEngine::new();

    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-SHELL-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("shell node should succeed");

    let output = &summary
        .timeline
        .iter()
        .find(|record| record.node_id == "shell_1")
        .expect("shell timeline item should exist")
        .output;
    assert_eq!(output["shell"], json!("sh"));
    assert_eq!(output["data"]["orderNo"], json!("SO-SHELL-1"));
    assert_eq!(output["data"]["tenantId"], json!("tenant-a"));
}

#[test]
fn resumes_task_branch_when_event_and_task_id_match() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let waiting_summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-5" },
                "body": { "orderNo": "SO-1005", "bizType": "manual_review" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    let task_id = waiting_summary
        .last_signal
        .as_ref()
        .and_then(|signal| signal.payload.get("taskId"))
        .cloned()
        .expect("task id should exist");

    let resumed = engine
        .resume(
            &definition,
            waiting_summary
                .resume_state
                .expect("waiting run should expose resume state"),
            json!({
                "event": "task.completed",
                "taskId": task_id,
                "status": "approved"
            }),
        )
        .expect("resume should complete");

    assert!(matches!(resumed.status, WorkflowRunStatus::Completed));
    assert_eq!(resumed.current_node_id.as_deref(), Some("end_1"));
    assert_eq!(
        resumed
            .timeline
            .iter()
            .rev()
            .find(|record| record.node_id == "manual_review_task")
            .expect("resumed task node should exist in timeline")
            .status,
        crate::core::runtime::ExecutionStatus::Success
    );
}

#[test]
fn rejects_resume_when_wait_event_mismatches() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let waiting_summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-6" },
                "body": { "orderNo": "SO-1006", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    let summary = engine
        .resume(
            &definition,
            waiting_summary
                .resume_state
                .expect("waiting run should expose resume state"),
            json!({
                "event": "wrong.callback",
                "correlationKey": "req-6"
            }),
        )
        .expect("resume should produce a failed summary");

    assert!(matches!(summary.status, WorkflowRunStatus::Failed));
    assert_eq!(
        summary
            .timeline
            .last()
            .and_then(|record| record.error_code.as_deref()),
        Some("RESUME_MISMATCH")
    );
}

#[test]
fn rejects_resume_when_correlation_key_mismatches() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let waiting_summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-7" },
                "body": { "orderNo": "SO-1007", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    let summary = engine
        .resume(
            &definition,
            waiting_summary
                .resume_state
                .expect("waiting run should expose resume state"),
            json!({
                "event": "rcs.callback",
                "correlationKey": "req-other"
            }),
        )
        .expect("resume should produce a failed summary");

    assert!(matches!(summary.status, WorkflowRunStatus::Failed));
    assert_eq!(
        summary
            .timeline
            .last()
            .and_then(|record| record.error_code.as_deref()),
        Some("RESUME_MISMATCH")
    );
}

#[test]
fn supports_extended_node_coverage_flow_with_subworkflow_and_respond() {
    let definition = load_coverage_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-coverage-1" },
                "body": {
                    "orderNo": "SO-COVER-1",
                    "customer": "customer-a",
                    "route": "subflow",
                    "needsDispatch": true
                }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(
        summary.timeline[1].node_type,
        crate::core::definition::NodeType::WebhookTrigger
    );
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("respond should emit a signal")
            .signal_type,
        "webhook_response"
    );
    assert_eq!(summary.state["subWorkflow"]["status"], json!("completed"));
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("respond should emit a signal")
            .payload["body"]["route"],
        json!("subflow")
    );
}

#[test]
fn emits_webhook_response_when_sync_webhook_flow_reaches_end() {
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": { "key": "sync-webhook-end-flow", "name": "Sync Webhook End Flow", "version": 1 },
        "trigger": { "type": "webhook", "responseMode": "sync" },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("sync webhook workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-sync-end-1" },
                "body": { "orderNo": "SO-SYNC-END-1", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.current_node_id.as_deref(), Some("end_1"));
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("sync webhook end should emit a response signal")
            .signal_type,
        "webhook_response"
    );
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("sync webhook end should emit a response signal")
            .payload,
        json!({
            "statusCode": 200,
            "body": {
                "orderNo": "SO-SYNC-END-1",
                "bizType": "auto_sort"
            }
        })
    );
}

#[test]
fn does_not_emit_webhook_response_for_async_ack_end_only_flow() {
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": { "key": "async-webhook-end-flow", "name": "Async Webhook End Flow", "version": 1 },
        "trigger": { "type": "webhook", "responseMode": "async_ack" },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("async webhook workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-async-end-1" },
                "body": { "orderNo": "SO-ASYNC-END-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert!(summary.last_signal.is_none());
}

#[test]
fn supports_if_else_false_branch_and_default_switch_branch() {
    let definition = load_coverage_flow_definition(&spawn_echo_http_server());
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-coverage-2" },
                "body": {
                    "orderNo": "SO-COVER-2",
                    "customer": "customer-b",
                    "route": "direct",
                    "needsDispatch": false
                }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.state["dispatch"]["skipped"], json!(true));
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("respond should emit a signal")
            .payload["body"]["dispatchSkipped"],
        json!(true)
    );
    assert_eq!(summary.state["subWorkflow"], serde_json::Value::Null);
}

#[test]
fn emits_running_summary_for_the_next_node_after_switch_branch_resolution() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let observer = Arc::new(RecordingObserver::default());
    let engine = WorkflowEngine::with_observer(observer.clone());

    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-switch-focus-1" },
                "body": { "orderNo": "SO-SWITCH-1", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Waiting));

    let summaries = observer.snapshot();
    let switch_running_summary = summaries
        .iter()
        .find(|item| {
            matches!(item.status, WorkflowRunStatus::Running)
                && item
                    .timeline
                    .last()
                    .map(|record| record.node_id.as_str() == "route_switch")
                    .unwrap_or(false)
        })
        .expect("running summary after switch should exist");

    assert_eq!(
        switch_running_summary.current_node_id.as_deref(),
        Some("dispatch_rcs_action")
    );
}

#[test]
fn cascades_waiting_sub_workflow_resume_to_parent_completion() {
    let definition: WorkflowDefinition = serde_json::from_str(include_str!("../../examples/subflow-wait-flow.json"))
        .expect("subflow wait workflow should deserialize");
    let engine = WorkflowEngine::new();
    let waiting = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-sub-1" },
                "body": { "orderNo": "SO-SUB-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("run should succeed");

    assert!(matches!(waiting.status, WorkflowRunStatus::Waiting));
    assert_eq!(waiting.current_node_id.as_deref(), Some("nested_workflow"));
    assert_eq!(
        waiting.last_signal.as_ref().expect("signal should exist").signal_type,
        "child.callback"
    );
    assert_eq!(waiting.state["nested"]["status"], json!("waiting"));
    assert!(waiting.state["nested"]["resumeState"].is_object());

    let resumed = engine
        .resume(
            &definition,
            waiting.resume_state.expect("resume state should exist"),
            json!({
                "event": "child.callback",
                "correlationKey": "SO-SUB-1",
                "status": "done"
            }),
        )
        .expect("resume should complete");

    assert!(matches!(resumed.status, WorkflowRunStatus::Completed));
    assert_eq!(resumed.current_node_id.as_deref(), Some("end_1"));
    assert_eq!(resumed.state["nested"]["status"], json!("completed"));
    assert_eq!(
        resumed
            .last_signal
            .as_ref()
            .expect("respond signal should exist")
            .signal_type,
        "webhook_response"
    );
}

#[test]
fn supports_code_node_state_patch_and_priority_branch() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../../examples/code-flow.json")).expect("code workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-code-1" },
                "body": { "orderNo": "SO-CODE-1", "qty": "6", "route": "priority" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.timeline[2].node_type, crate::core::definition::NodeType::Code);
    assert_eq!(summary.state["code"]["normalizedQty"], json!(6));
    assert_eq!(summary.state["code"]["branch"], json!("priority"));
    assert_eq!(summary.state["code"]["requestId"], json!("req-code-1"));
    assert_eq!(summary.state["decision"]["handledBy"], json!("priority"));
    assert_eq!(summary.timeline[2].logs.len(), 1);
    assert_eq!(summary.timeline[2].logs[0].level, "log");
    assert!(summary.timeline[2].logs[0].message.contains("req-code-1"));
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("respond should emit a signal")
            .payload["body"]["handledBy"],
        json!("priority")
    );
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("respond should emit a signal")
            .payload["body"]["requestId"],
        json!("req-code-1")
    );
}

#[test]
fn supports_code_node_default_branch() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../../examples/code-flow.json")).expect("code workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-code-2" },
                "body": { "orderNo": "SO-CODE-2", "qty": 2, "route": "normal" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.state["code"]["branch"], json!("default"));
    assert_eq!(summary.state["code"]["requestId"], json!("req-code-2"));
    assert_eq!(summary.state["decision"]["handledBy"], json!("default"));
    assert_eq!(summary.timeline[2].logs.len(), 1);
    assert_eq!(
        summary
            .last_signal
            .as_ref()
            .expect("respond should emit a signal")
            .payload["body"]["branch"],
        json!("default")
    );
}

#[test]
fn rejects_code_node_when_timeout_is_exceeded() {
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": { "key": "code-timeout-flow", "name": "Code Timeout Flow", "version": 1 },
        "trigger": { "type": "manual" },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "run_code",
                "type": "code",
                "name": "Run Code",
                "timeoutMs": 25,
                "config": {
                    "language": "js",
                    "source": "await new Promise((resolve) => setTimeout(resolve, 120)); return { ok: true };"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "run_code" },
            { "from": "run_code", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("timeout workflow should deserialize");
    let engine = WorkflowEngine::new();

    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-TIMEOUT-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("timeout should return a failed summary");

    assert!(matches!(summary.status, WorkflowRunStatus::Failed));
    assert_eq!(
        summary
            .timeline
            .last()
            .and_then(|record| record.error_code.as_deref()),
        Some("TIMEOUT")
    );
}

#[test]
fn supports_inline_typescript_code_node() {
    if !node_supports_typescript_code() {
        return;
    }

    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": { "key": "code-typescript-inline-flow", "name": "Code TypeScript Inline Flow", "version": 1 },
        "trigger": { "type": "manual" },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "run_code",
                "type": "code",
                "name": "Run TypeScript",
                "inputMapping": {
                    "qty": "{{input.qty}}",
                    "orderNo": "{{input.orderNo}}"
                },
                "config": {
                    "language": "typescript",
                    "source": "const quantity: number = Number(params.qty ?? 0);\nconst payload: { orderNo: string | null; normalizedQty: number } = {\n  orderNo: params.orderNo ?? null,\n  normalizedQty: quantity * 2,\n};\nreturn { output: payload, statePatch: { code: payload } };"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "run_code" },
            { "from": "run_code", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("typescript workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-TS-1", "qty": 3 }
            }),
            RunEnvironment::default(),
        )
        .expect("inline typescript should run");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.timeline[1].output["orderNo"], json!("SO-TS-1"));
    assert_eq!(summary.timeline[1].output["normalizedQty"], json!(6));
    assert_eq!(summary.state["code"]["normalizedQty"], json!(6));
}

#[test]
fn supports_code_node_source_path_script() {
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": { "key": "code-source-path-flow", "name": "Code Source Path Flow", "version": 1 },
        "trigger": { "type": "manual" },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "run_code",
                "type": "code",
                "name": "Run Code",
                "inputMapping": { "orderNo": "{{input.orderNo}}" },
                "config": { "language": "js", "sourcePath": "examples/code-source-handler.js" }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "run_code" },
            { "from": "run_code", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("source path workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-SOURCE-1" }
            }),
            RunEnvironment::default(),
        )
        .expect("source path script should run");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.timeline[1].node_type, crate::core::definition::NodeType::Code);
    assert_eq!(summary.timeline[1].output["source"], json!("file"));
    assert_eq!(summary.timeline[1].output["orderNo"], json!("SO-SOURCE-1"));
}

#[test]
fn supports_code_node_module_export_name_with_base_dir() {
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": {
            "key": "code-module-export-flow",
            "name": "Code Module Export Flow",
            "version": 1
        },
        "trigger": { "type": "manual" },
        "inputSchema": { "type": "object" },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            {
                "id": "run_code",
                "type": "code",
                "name": "Run Code",
                "inputMapping": {
                    "orderNo": "{{input.orderNo}}",
                    "route": "{{input.route}}"
                },
                "config": {
                    "language": "js",
                    "baseDir": "examples/code-modules",
                    "modulePath": "reusable-handler.mjs",
                    "exportName": "branchByPriority"
                }
            },
            {
                "id": "mark_priority",
                "type": "set_state",
                "name": "Mark Priority",
                "config": { "path": "decision" },
                "inputMapping": { "value": { "handledBy": "priority-module" } }
            },
            {
                "id": "mark_default",
                "type": "set_state",
                "name": "Mark Default",
                "config": { "path": "decision" },
                "inputMapping": { "value": { "handledBy": "default-module" } }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "run_code" },
            { "from": "run_code", "to": "mark_priority", "label": "priority", "priority": 100 },
            { "from": "run_code", "to": "mark_default", "branchType": "default", "priority": 1 },
            { "from": "mark_priority", "to": "end_1" },
            { "from": "mark_default", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("module export workflow should deserialize");
    let engine = WorkflowEngine::new();
    let summary = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-MODULE-1", "route": "priority" }
            }),
            RunEnvironment::default(),
        )
        .expect("module export should run");

    assert!(matches!(summary.status, WorkflowRunStatus::Completed));
    assert_eq!(summary.timeline[1].output["source"], json!("named-export"));
    assert_eq!(summary.state["moduleResult"]["branch"], json!("priority"));
    assert_eq!(summary.state["decision"]["handledBy"], json!("priority-module"));
    assert_eq!(summary.timeline[1].logs[0].level, "info");
    assert!(summary.timeline[1].logs[0].message.contains("SO-MODULE-1"));
}
