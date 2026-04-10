use std::sync::Arc;

use serde_json::json;

use crate::definition::WorkflowDefinition;
use crate::engine::WorkflowEngine;
use crate::runtime::{RunEnvironment, WorkflowRunStatus};
use crate::store::{InMemoryRunStore, WorkflowRunStore, WorkflowRunner};

#[test]
fn stores_waiting_snapshot_and_resumes_by_run_id() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/subflow-wait-flow.json"))
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
