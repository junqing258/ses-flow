use serde_json::json;

use crate::core::runtime::{NodeExecutionContext, RunEnvironment};
use crate::services::WorkflowServices;

#[test]
fn default_services_register_mock_task_handler() {
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

    let task = services
        .task_handlers
        .resolve("manual_review")
        .expect("default task handler should exist")
        .create(&json!({"orderNo": "SO-1"}), &context)
        .expect("task should succeed");

    assert_eq!(task["status"], json!("created"));
}
