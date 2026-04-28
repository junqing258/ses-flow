use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use futures_util::StreamExt;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::app::{AppError, ConcurrencyConfig, EditSessionDraftOperation, OverflowPolicy, WorkflowApp};
use crate::core::definition::WorkflowDefinition;
use crate::core::runtime::WorkflowRunStatus;
use crate::store::{InMemoryCatalogStore, InMemoryRunStore};

fn sample_workflow(key: &str) -> WorkflowDefinition {
    serde_json::from_value(json!({
        "meta": {
            "key": key,
            "name": "Sample Flow",
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
    }))
    .expect("sample workflow should deserialize")
}

fn wait_workflow(key: &str) -> WorkflowDefinition {
    serde_json::from_value(json!({
        "meta": {
            "key": key,
            "name": "Wait Flow",
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
                "config": {
                    "event": "agv.arrived"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "wait_1" },
            { "from": "wait_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("wait workflow should deserialize")
}

fn delayed_fetch_workflow(key: &str, url: &str) -> WorkflowDefinition {
    serde_json::from_value(json!({
        "meta": {
            "key": key,
            "name": "Delayed Fetch Flow",
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
                "config": {
                    "url": url,
                    "method": "GET"
                }
            },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "fetch_1" },
            { "from": "fetch_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("delayed fetch workflow should deserialize")
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

#[test]
fn registers_workflow_and_lists_detail() {
    let app = WorkflowApp::new();
    let definition = sample_workflow("app-test-flow");

    let registration = app
        .register_workflow(
            Some("ws-test".to_string()),
            Some("Test Workspace".to_string()),
            None,
            definition,
            None,
        )
        .expect("workflow should register");

    let detail = app
        .get_workflow(&registration.workflow_id)
        .expect("workflow detail should load");

    assert_eq!(detail.summary.workflow_key, "app-test-flow");
    assert_eq!(detail.summary.owner_name.as_deref(), Some("Test Workspace"));
    assert_eq!(app.list_workflows().expect("workflow list should load").len(), 1);
}

#[test]
fn creates_and_updates_edit_session() {
    let app = WorkflowApp::new();

    let session = app
        .create_edit_session(
            Some("ws-edit".to_string()),
            Some("wf-1".to_string()),
            sample_workflow("edit-session-flow"),
            Some(json!({"version": 1})),
        )
        .expect("edit session should be created");

    let updated = app
        .update_edit_session(
            &session.session_id,
            Some("wf-2".to_string()),
            sample_workflow("edit-session-flow"),
            Some(json!({"version": 2})),
        )
        .expect("edit session should update");

    assert_eq!(updated.workflow_id.as_deref(), Some("wf-2"));
    assert_eq!(updated.editor_document, Some(json!({"version": 2})));
}

#[test]
fn applies_edit_session_operations_with_node_removal_cascade() {
    let app = WorkflowApp::new();

    let session = app
        .create_edit_session(
            Some("ws-edit".to_string()),
            Some("wf-1".to_string()),
            serde_json::from_value(json!({
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
            }))
            .expect("workflow should deserialize"),
            Some(json!({
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
                    "id": "wf-1",
                    "name": "Edit Session Flow",
                    "status": "draft",
                    "version": "1"
                }
            })),
        )
        .expect("edit session should be created");

    let updated = app
        .apply_edit_session_operations(
            &session.session_id,
            None,
            vec![EditSessionDraftOperation::RemoveNodeCascade {
                node_id: "fetch_1".to_string(),
            }],
        )
        .expect("edit session operations should apply");

    assert_eq!(
        updated
            .workflow
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["start_1", "end_1"]
    );
    assert_eq!(
        updated
            .workflow
            .transitions
            .iter()
            .map(|transition| (transition.from.as_str(), transition.to.as_str()))
            .collect::<Vec<_>>(),
        vec![("start_1", "end_1")]
    );

    let editor_document = updated.editor_document.expect("editor document should still exist");
    assert_eq!(
        editor_document["graph"]["nodes"]
            .as_array()
            .expect("graph nodes should exist")
            .iter()
            .filter_map(|node| node["id"].as_str())
            .collect::<Vec<_>>(),
        vec!["start_1", "end_1"]
    );
    assert!(
        editor_document["graph"]["edges"]
            .as_array()
            .expect("graph edges should exist")
            .iter()
            .any(|edge| { edge["source"].as_str() == Some("start_1") && edge["target"].as_str() == Some("end_1") })
    );
    assert!(editor_document["graph"]["panels"]["fetch_1"].is_null());
    assert!(editor_document["editor"]["selectedNodeId"].is_null());
}

#[test]
fn applies_edit_session_operations_for_node_config_and_edge_mutations() {
    let app = WorkflowApp::new();

    let session = app
        .create_edit_session(
            Some("ws-edit".to_string()),
            Some("wf-1".to_string()),
            serde_json::from_value(json!({
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
            }))
            .expect("workflow should deserialize"),
            Some(json!({
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
                    "id": "wf-1",
                    "name": "Edit Session Flow",
                    "status": "draft",
                    "version": "1"
                }
            })),
        )
        .expect("edit session should be created");

    let updated = app
        .apply_edit_session_operations(
            &session.session_id,
            None,
            vec![
                EditSessionDraftOperation::UpdateNodeConfig {
                    node_id: "fetch_1".to_string(),
                    config: json!({
                        "url": "https://api.example.com",
                        "method": "POST"
                    }),
                },
                EditSessionDraftOperation::AddEdge {
                    source: "fetch_1".to_string(),
                    target: "wait_1".to_string(),
                    source_handle: Some("out".to_string()),
                    target_handle: Some("in".to_string()),
                },
                EditSessionDraftOperation::UpdateEdge {
                    edge_id: "edge:fetch_1:out->wait_1:in".to_string(),
                    updates: json!({
                        "target": "end_1",
                        "targetHandle": "in",
                        "label": "success",
                        "priority": 5
                    }),
                },
            ],
        )
        .expect("edit session operations should apply");

    let fetch_node = updated
        .workflow
        .nodes
        .iter()
        .find(|node| node.id == "fetch_1")
        .expect("fetch node should exist");
    assert_eq!(
        fetch_node.config,
        json!({
            "url": "https://api.example.com",
            "method": "POST"
        })
    );
    assert!(updated.workflow.transitions.iter().any(|transition| {
        transition.from == "fetch_1"
            && transition.to == "end_1"
            && transition.label.as_deref() == Some("success")
            && transition.priority == Some(5)
    }));
    assert!(
        !updated
            .workflow
            .transitions
            .iter()
            .any(|transition| { transition.from == "fetch_1" && transition.to == "wait_1" })
    );

    let editor_document = updated.editor_document.expect("editor document should exist");
    assert!(
        editor_document["graph"]["edges"]
            .as_array()
            .expect("graph edges should exist")
            .iter()
            .any(|edge| {
                edge["id"].as_str() == Some("edge:fetch_1:out->end_1:in") && edge["label"].as_str() == Some("success")
            })
    );
    assert!(
        !editor_document["graph"]["edges"]
            .as_array()
            .expect("graph edges should exist")
            .iter()
            .any(|edge| edge["id"].as_str() == Some("edge:fetch_1:out->wait_1:in"))
    );

    let removed = app
        .apply_edit_session_operations(
            &session.session_id,
            None,
            vec![EditSessionDraftOperation::RemoveEdge {
                edge_id: "edge:fetch_1:out->end_1:in".to_string(),
            }],
        )
        .expect("remove edge operation should apply");

    assert!(
        !removed
            .workflow
            .transitions
            .iter()
            .any(|transition| { transition.from == "fetch_1" && transition.to == "end_1" })
    );
    let editor_document = removed.editor_document.expect("editor document should exist");
    assert!(
        !editor_document["graph"]["edges"]
            .as_array()
            .expect("graph edges should exist")
            .iter()
            .any(|edge| edge["id"].as_str() == Some("edge:fetch_1:out->end_1:in"))
    );
}

#[tokio::test]
async fn emits_session_change_notifications() {
    let app = WorkflowApp::new();
    let session = app
        .create_edit_session(None, None, sample_workflow("session-events-flow"), None)
        .expect("edit session should be created");

    let mut stream = app.subscribe_edit_session_events(&session.session_id);
    let initial = stream.next().await.expect("initial notification should exist");
    assert_eq!(initial.event_type, "stream.connected");

    let updated = app
        .update_edit_session(
            &session.session_id,
            Some("wf-events".to_string()),
            sample_workflow("session-events-flow"),
            Some(json!({"draft": true})),
        )
        .expect("edit session should update");

    let next = stream.next().await.expect("session update notification should exist");
    assert_eq!(next.event_type, "session.changed");
    assert_eq!(next.session_id.as_deref(), Some(updated.session_id.as_str()));
}

#[tokio::test]
async fn starts_workflow_and_persists_completed_summary() {
    let app = WorkflowApp::new();
    let registration = app
        .register_workflow(
            Some("ws-run".to_string()),
            Some("Run Workspace".to_string()),
            None,
            sample_workflow("run-flow"),
            None,
        )
        .expect("workflow should register");

    let summary = app
        .start_workflow(
            &registration.workflow_id,
            json!({
                "body": {
                    "orderNo": "SO-APP-1"
                }
            }),
            Default::default(),
        )
        .await
        .expect("workflow should start");

    let final_summary = wait_for_terminal_summary(&app, &summary.run_id).await;
    assert!(matches!(final_summary.status, WorkflowRunStatus::Completed));
    assert_eq!(final_summary.workflow_key, "run-flow");
}

#[tokio::test]
async fn resumes_waiting_run_after_registry_restart_from_persisted_summary() {
    let store = Arc::new(InMemoryRunStore::new());
    let catalog = Arc::new(InMemoryCatalogStore::new());
    let first_app = WorkflowApp::with_store_and_catalog(store.clone(), catalog.clone());
    let registration = first_app
        .register_workflow(
            Some("ws-run".to_string()),
            Some("Run Workspace".to_string()),
            None,
            wait_workflow("restart-resume-flow"),
            None,
        )
        .expect("workflow should register");

    let started = first_app
        .start_workflow(
            &registration.workflow_id,
            json!({
                "body": {
                    "stationId": "station-1"
                }
            }),
            Default::default(),
        )
        .await
        .expect("workflow should start");
    let waiting = wait_for_waiting_summary(&first_app, &started.run_id).await;
    assert!(matches!(waiting.status, WorkflowRunStatus::Waiting));
    assert_eq!(waiting.current_node_id.as_deref(), Some("wait_1"));

    let restarted_app = WorkflowApp::with_store_and_catalog(store, catalog);
    let accepted = restarted_app
        .resume_workflow(
            &waiting.run_id,
            json!({
                "event": "agv.arrived",
                "stationId": "station-1"
            }),
        )
        .await
        .expect("resume should be accepted after registry restart");
    assert!(matches!(accepted.status, WorkflowRunStatus::Running));

    let final_summary = wait_for_terminal_summary(&restarted_app, &waiting.run_id).await;
    assert!(matches!(final_summary.status, WorkflowRunStatus::Completed));
}

#[tokio::test]
async fn rejects_second_start_when_workflow_concurrency_limit_is_reached() {
    let app = WorkflowApp::with_concurrency_config(ConcurrencyConfig {
        max_global: 5,
        queue_timeout_secs: 1,
        overflow_policy: OverflowPolicy::Reject,
        per_workflow: crate::app::PerWorkflowConcurrencyConfig {
            default_max: 1,
            overrides: Default::default(),
        },
    });
    let delayed_server = spawn_delayed_http_server(Duration::from_millis(250));
    let registration = app
        .register_workflow(
            Some("ws-run".to_string()),
            Some("Run Workspace".to_string()),
            None,
            delayed_fetch_workflow("limited-flow", &delayed_server),
            None,
        )
        .expect("workflow should register");

    let first = app
        .start_workflow(&registration.workflow_id, json!({}), Default::default())
        .await
        .expect("first workflow should start");

    let second = app
        .start_workflow(&registration.workflow_id, json!({}), Default::default())
        .await;

    assert!(matches!(second, Err(AppError::Throttled(_))));

    let final_summary = wait_for_terminal_summary(&app, &first.run_id).await;
    assert!(matches!(final_summary.status, WorkflowRunStatus::Completed));
}

#[tokio::test]
async fn reuses_active_workflow_run_for_same_unique_key() {
    let app = WorkflowApp::new();
    let registration = app
        .register_workflow(
            Some("ws-run".to_string()),
            Some("Run Workspace".to_string()),
            None,
            wait_workflow("idempotent-unique-key-flow"),
            None,
        )
        .expect("workflow should register");

    let trigger = json!({
        "headers": { "requestId": "req-idempotent-1" },
        "body": { "uniqueKey": "order:SO-IDEMPOTENT-1" }
    });
    let first = app
        .start_workflow(&registration.workflow_id, trigger.clone(), Default::default())
        .await
        .expect("first workflow should start");
    let waiting = wait_for_waiting_summary(&app, &first.run_id).await;
    assert_eq!(waiting.run_id, first.run_id);

    let second = app
        .start_workflow(&registration.workflow_id, trigger.clone(), Default::default())
        .await
        .expect("duplicate active workflow should return existing run");

    assert_eq!(second.run_id, first.run_id);
    assert!(matches!(second.status, WorkflowRunStatus::Waiting));

    app.resume_workflow(
        &first.run_id,
        json!({
            "event": "agv.arrived",
            "stationId": "station-1"
        }),
    )
    .await
    .expect("resume should be accepted");
    let completed = wait_for_terminal_summary(&app, &first.run_id).await;
    assert!(matches!(completed.status, WorkflowRunStatus::Completed));

    let third = app
        .start_workflow(&registration.workflow_id, trigger, Default::default())
        .await
        .expect("completed unique key should be allowed to start again");

    assert_ne!(third.run_id, first.run_id);
}

#[tokio::test]
async fn reuses_active_workflow_run_before_concurrency_reject_for_same_unique_key() {
    let app = WorkflowApp::with_concurrency_config(ConcurrencyConfig {
        max_global: 5,
        queue_timeout_secs: 1,
        overflow_policy: OverflowPolicy::Reject,
        per_workflow: crate::app::PerWorkflowConcurrencyConfig {
            default_max: 1,
            overrides: Default::default(),
        },
    });
    let delayed_server = spawn_delayed_http_server(Duration::from_millis(250));
    let registration = app
        .register_workflow(
            Some("ws-run".to_string()),
            Some("Run Workspace".to_string()),
            None,
            delayed_fetch_workflow("idempotent-limited-flow", &delayed_server),
            None,
        )
        .expect("workflow should register");

    let trigger = json!({
        "headers": { "requestId": "req-idempotent-limited-1" },
        "body": { "uniqueKey": "order:SO-IDEMPOTENT-LIMITED-1" }
    });
    let first = app
        .start_workflow(&registration.workflow_id, trigger.clone(), Default::default())
        .await
        .expect("first workflow should start");

    let second = app
        .start_workflow(&registration.workflow_id, trigger, Default::default())
        .await
        .expect("duplicate active workflow should return existing run before concurrency rejection");

    assert_eq!(second.run_id, first.run_id);

    let final_summary = wait_for_terminal_summary(&app, &first.run_id).await;
    assert!(matches!(final_summary.status, WorkflowRunStatus::Completed));
}

#[tokio::test]
async fn queues_second_start_until_first_run_releases_its_permit() {
    let app = WorkflowApp::with_concurrency_config(ConcurrencyConfig {
        max_global: 5,
        queue_timeout_secs: 2,
        overflow_policy: OverflowPolicy::Queue,
        per_workflow: crate::app::PerWorkflowConcurrencyConfig {
            default_max: 1,
            overrides: Default::default(),
        },
    });
    let delayed_server = spawn_delayed_http_server(Duration::from_millis(250));
    let registration = app
        .register_workflow(
            Some("ws-run".to_string()),
            Some("Run Workspace".to_string()),
            None,
            delayed_fetch_workflow("queued-flow", &delayed_server),
            None,
        )
        .expect("workflow should register");

    let first = app
        .start_workflow(&registration.workflow_id, json!({}), Default::default())
        .await
        .expect("first workflow should start");

    let started_at = Instant::now();
    let second = app
        .start_workflow(&registration.workflow_id, json!({}), Default::default())
        .await
        .expect("second workflow should eventually start");

    assert!(started_at.elapsed() >= Duration::from_millis(200));

    let first_summary = wait_for_terminal_summary(&app, &first.run_id).await;
    let second_summary = wait_for_terminal_summary(&app, &second.run_id).await;
    assert!(matches!(first_summary.status, WorkflowRunStatus::Completed));
    assert!(matches!(second_summary.status, WorkflowRunStatus::Completed));
}

async fn wait_for_terminal_summary(app: &WorkflowApp, run_id: &str) -> crate::core::runtime::WorkflowRunSummary {
    for _ in 0..40 {
        if let Some(summary) = app.get_summary(run_id).expect("summary should load") {
            if matches!(
                summary.status,
                WorkflowRunStatus::Completed | WorkflowRunStatus::Failed | WorkflowRunStatus::Terminated
            ) {
                return summary;
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    panic!("workflow run did not reach a terminal state in time");
}

async fn wait_for_waiting_summary(app: &WorkflowApp, run_id: &str) -> crate::core::runtime::WorkflowRunSummary {
    for _ in 0..40 {
        if let Some(summary) = app.get_summary(run_id).expect("summary should load") {
            if matches!(summary.status, WorkflowRunStatus::Waiting) {
                return summary;
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    panic!("workflow run did not reach waiting state in time");
}
