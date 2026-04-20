use futures_util::StreamExt;
use serde_json::json;
use tokio::time::{Duration, sleep};

use crate::app::{EditSessionDraftOperation, WorkflowApp};
use crate::core::definition::WorkflowDefinition;
use crate::core::runtime::WorkflowRunStatus;

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
