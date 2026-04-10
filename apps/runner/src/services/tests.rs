use serde_json::json;

use crate::core::runtime::{NodeExecutionContext, RunEnvironment};
use crate::services::WorkflowServices;

#[test]
fn default_services_register_mock_handlers() {
    let services = WorkflowServices::with_defaults();
    let context = NodeExecutionContext {
        run_id: "run-1",
        workflow_key: "workflow.demo",
        workflow_version: 1,
        trigger: &json!({}),
        input: &json!({}),
        state: &json!({}),
        env: &RunEnvironment::default(),
    };

    let fetch = services
        .fetch_connectors
        .resolve("oms.getOrder")
        .expect("default fetch connector should exist")
        .fetch(&json!({"orderNo": "SO-1"}), &context)
        .expect("fetch should succeed");
    let action = services
        .action_handlers
        .resolve("rcs.dispatch")
        .expect("default action handler should exist")
        .execute(&json!({"orderNo": "SO-1"}), &context)
        .expect("action should succeed");
    let task = services
        .task_handlers
        .resolve("manual_review")
        .expect("default task handler should exist")
        .create(&json!({"orderNo": "SO-1"}), &context)
        .expect("task should succeed");

    assert_eq!(fetch["orderNo"], json!("SO-1"));
    assert_eq!(action["accepted"], json!(true));
    assert_eq!(task["status"], json!("created"));
}
