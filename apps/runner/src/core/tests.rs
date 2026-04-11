use std::sync::{Arc, Mutex};

use serde_json::json;

use super::definition::WorkflowDefinition;
use super::engine::WorkflowEngine;
use super::runtime::{
    RunEnvironment, WorkflowRunObserver, WorkflowRunStatus, WorkflowRunSummary,
};
use crate::services::{FetchConnector, WorkflowServices};

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

struct CustomOrderConnector;

impl FetchConnector for CustomOrderConnector {
    fn name(&self) -> &'static str {
        "oms.getOrder"
    }

    fn fetch(
        &self,
        request: &serde_json::Value,
        _context: &crate::runtime::NodeExecutionContext<'_>,
    ) -> Result<serde_json::Value, crate::error::RunnerError> {
        Ok(json!({
            "source": "custom",
            "orderNo": request.get("orderNo").cloned().unwrap_or(serde_json::Value::Null)
        }))
    }
}

#[test]
fn waits_on_task_branch_for_manual_review() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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
    assert_eq!(
        summary.current_node_id.as_deref(),
        Some("manual_review_task")
    );
}

#[test]
fn waits_on_callback_branch_for_auto_sort() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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
    assert_eq!(
        summary.current_node_id.as_deref(),
        Some("wait_dispatch_callback")
    );
    assert_eq!(
        summary.state["orderSnapshot"]["data"]["orderNo"],
        json!("SO-1002")
    );
}

#[test]
fn resumes_waiting_callback_to_completion() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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
            .last()
            .expect("timeline should not be empty")
            .output,
        json!({
            "correlationKey": "req-3",
            "event": "rcs.callback",
            "status": "done",
            "orderNo": "SO-1003"
        })
    );
}

#[test]
fn supports_custom_service_registration() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
    let mut services = WorkflowServices::with_defaults();
    services.fetch_connectors.register(CustomOrderConnector);
    let engine = WorkflowEngine::with_services(services);

    let summary = engine
        .run(
            &definition,
            json!({
                "headers": { "requestId": "req-4" },
                "body": { "orderNo": "SO-1004", "bizType": "auto_sort" }
            }),
            RunEnvironment::default(),
        )
        .expect("workflow run should succeed");

    assert_eq!(
        summary.state["orderSnapshot"]["data"]["source"],
        json!("custom")
    );
    assert_eq!(
        summary.state["orderSnapshot"]["data"]["orderNo"],
        json!("SO-1004")
    );
}

#[test]
fn resumes_task_branch_when_event_and_task_id_match() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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
}

#[test]
fn rejects_resume_when_wait_event_mismatches() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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

    let error = engine
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
        .expect_err("resume should be rejected");

    assert!(matches!(
        error,
        crate::error::RunnerError::ResumeValidation(_)
    ));
}

#[test]
fn rejects_resume_when_correlation_key_mismatches() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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

    let error = engine
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
        .expect_err("resume should be rejected");

    assert!(matches!(
        error,
        crate::error::RunnerError::ResumeValidation(_)
    ));
}

#[test]
fn supports_extended_node_coverage_flow_with_subworkflow_and_respond() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/coverage-flow.json"))
            .expect("coverage workflow should deserialize");
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
        crate::definition::NodeType::WebhookTrigger
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
fn supports_if_else_false_branch_and_default_switch_branch() {
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/coverage-flow.json"))
            .expect("coverage workflow should deserialize");
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
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))
            .expect("example workflow should deserialize");
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
    let definition: WorkflowDefinition =
        serde_json::from_str(include_str!("../examples/subflow-wait-flow.json"))
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
        waiting
            .last_signal
            .as_ref()
            .expect("signal should exist")
            .signal_type,
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
        serde_json::from_str(include_str!("../examples/code-flow.json"))
            .expect("code workflow should deserialize");
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
    assert_eq!(
        summary.timeline[2].node_type,
        crate::definition::NodeType::Code
    );
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
        serde_json::from_str(include_str!("../examples/code-flow.json"))
            .expect("code workflow should deserialize");
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

    let error = engine
        .run(
            &definition,
            json!({
                "body": { "orderNo": "SO-TIMEOUT-1" }
            }),
            RunEnvironment::default(),
        )
        .expect_err("timeout should fail");

    assert!(matches!(
        error,
        crate::error::RunnerError::CodeExecution(message) if message.contains("timeout")
    ));
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
    assert_eq!(
        summary.timeline[1].node_type,
        crate::definition::NodeType::Code
    );
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
    assert_eq!(
        summary.state["decision"]["handledBy"],
        json!("priority-module")
    );
    assert_eq!(summary.timeline[1].logs[0].level, "info");
    assert!(summary.timeline[1].logs[0].message.contains("SO-MODULE-1"));
}
