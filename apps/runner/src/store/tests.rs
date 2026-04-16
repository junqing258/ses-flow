use crate::app::WorkflowRunner;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use serde_json::json;

use crate::core::definition::WorkflowDefinition;
use crate::core::engine::WorkflowEngine;
use crate::core::runtime::{RunEnvironment, WorkflowRunStatus};
use crate::store::{InMemoryRunStore, WorkflowRunStore};

#[test]
fn stores_waiting_snapshot_and_resumes_by_run_id() {
    let definition = load_sorting_flow_definition(&spawn_echo_http_server());
    let store = Arc::new(InMemoryRunStore::new());
    let runner = WorkflowRunner::new(WorkflowEngine::new(), store.clone());

    let waiting = runner
        .run(
            &definition,
            json!({
                "headers": {
                    "requestId": "req-store-1"
                },
                "body": {
                    "orderNo": "SO-STORE-1",
                    "bizType": "auto_sort"
                }
            }),
            RunEnvironment::default(),
        )
        .expect("run should succeed");

    assert!(matches!(waiting.status, WorkflowRunStatus::Waiting));
    assert!(
        store
            .load_snapshot(&waiting.run_id)
            .expect("load snapshot should succeed")
            .is_some()
    );

    let completed = runner
        .resume_by_run_id(
            &definition,
            &waiting.run_id,
            json!({
                "event": "rcs.callback",
                "correlationKey": "req-store-1",
                "orderNo": "SO-STORE-1",
                "status": "done"
            }),
        )
        .expect("resume should succeed");

    assert!(matches!(completed.status, WorkflowRunStatus::Completed));
    assert!(
        store
            .load_snapshot(&waiting.run_id)
            .expect("load snapshot should succeed")
            .is_none()
    );
}

#[test]
fn resumes_parent_run_id_for_waiting_sub_workflow() {
    let definition: WorkflowDefinition = serde_json::from_str(include_str!("../../examples/subflow-wait-flow.json"))
        .expect("subflow wait workflow should deserialize");
    let store = Arc::new(InMemoryRunStore::new());
    let runner = WorkflowRunner::new(WorkflowEngine::new(), store.clone());

    let waiting = runner
        .run(
            &definition,
            json!({
                "headers": {
                    "requestId": "req-store-sub-1"
                },
                "body": {
                    "orderNo": "SO-SUB-1"
                }
            }),
            RunEnvironment::default(),
        )
        .expect("run should succeed");

    assert!(matches!(waiting.status, WorkflowRunStatus::Waiting));
    assert_eq!(waiting.current_node_id.as_deref(), Some("nested_workflow"));

    let completed = runner
        .resume_by_run_id(
            &definition,
            &waiting.run_id,
            json!({
                "event": "child.callback",
                "correlationKey": "SO-SUB-1",
                "status": "done"
            }),
        )
        .expect("resume should succeed");

    assert!(matches!(completed.status, WorkflowRunStatus::Completed));
    assert_eq!(completed.state["nested"]["status"], json!("completed"));
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

            loop {
                let read = stream.read(&mut chunk).expect("echo test server should read request");
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
