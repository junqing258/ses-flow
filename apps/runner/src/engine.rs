use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::definition::{NodeType, TransitionDefinition, WorkflowDefinition};
use crate::error::RunnerError;
use crate::executors::ExecutorRegistry;
use crate::runtime::{
    ExecutionStatus, NodeExecutionContext, NodeExecutionRecord, RunEnvironment,
    WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary,
};
use crate::services::WorkflowServices;
use crate::template::{merge_state, nested_state_patch};

pub struct WorkflowEngine {
    registry: ExecutorRegistry,
    services: WorkflowServices,
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self::with_services(WorkflowServices::with_defaults())
    }

    pub fn with_services(services: WorkflowServices) -> Self {
        Self {
            registry: ExecutorRegistry::with_defaults(Arc::new(services.clone())),
            services,
        }
    }

    pub fn run(
        &self,
        definition: &WorkflowDefinition,
        trigger: Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        definition.validate()?;
        let run_id = new_run_id();
        let current_node_id = definition.start_node()?.id.clone();
        let current_input = trigger
            .get("body")
            .cloned()
            .unwrap_or_else(|| trigger.clone());

        self.execute_from(
            definition,
            run_id,
            trigger,
            env,
            json!({}),
            Vec::new(),
            current_node_id,
            current_input,
        )
    }

    pub fn resume(
        &self,
        definition: &WorkflowDefinition,
        snapshot: WorkflowRunSnapshot,
        resume_input: Value,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        definition.validate()?;

        if snapshot.workflow_key != definition.meta.key {
            return Err(RunnerError::Validation(format!(
                "resume snapshot workflow key mismatch: expected {}, got {}",
                definition.meta.key, snapshot.workflow_key
            )));
        }

        if snapshot.workflow_version != definition.meta.version {
            return Err(RunnerError::Validation(format!(
                "resume snapshot workflow version mismatch: expected {}, got {}",
                definition.meta.version, snapshot.workflow_version
            )));
        }

        let waiting_node = definition
            .node(&snapshot.current_node_id)
            .ok_or_else(|| RunnerError::MissingNode(snapshot.current_node_id.clone()))?;
        if waiting_node.node_type == NodeType::SubWorkflow {
            return self.resume_sub_workflow(definition, waiting_node, snapshot, resume_input);
        }
        self.validate_resume_input(waiting_node, &snapshot, &resume_input)?;
        let outgoing = definition.transitions_from(&waiting_node.id);
        let next = self.resolve_transition(&outgoing, None)?;

        self.execute_from(
            definition,
            snapshot.run_id,
            snapshot.trigger,
            snapshot.env,
            snapshot.state,
            snapshot.timeline,
            next.to.clone(),
            resume_input,
        )
    }

    fn resolve_transition<'a>(
        &self,
        transitions: &'a [&TransitionDefinition],
        branch_key: Option<&str>,
    ) -> Result<&'a TransitionDefinition, RunnerError> {
        if transitions.is_empty() {
            return Err(RunnerError::Transition(
                "current node has no outgoing transitions".to_string(),
            ));
        }

        if let Some(branch_key) = branch_key {
            if let Some(label_match) = transitions
                .iter()
                .copied()
                .find(|transition| transition.label.as_deref() == Some(branch_key))
            {
                return Ok(label_match);
            }

            if let Some(condition_match) = transitions
                .iter()
                .copied()
                .find(|transition| transition.condition.as_deref() == Some(branch_key))
            {
                return Ok(condition_match);
            }
        }

        if let Some(default_branch) = transitions.iter().copied().find(|transition| {
            matches!(transition.branch_type.as_deref(), Some("default"))
                || matches!(transition.label.as_deref(), Some("default"))
        }) {
            return Ok(default_branch);
        }

        if transitions.len() == 1 {
            return Ok(transitions[0]);
        }

        Err(RunnerError::Transition(format!(
            "no transition matched the current branch: {}",
            branch_key.unwrap_or("<none>")
        )))
    }

    fn validate_resume_input(
        &self,
        waiting_node: &crate::definition::NodeDefinition,
        snapshot: &WorkflowRunSnapshot,
        resume_input: &Value,
    ) -> Result<(), RunnerError> {
        match waiting_node.node_type {
            NodeType::Wait => {
                let expected_event = waiting_node
                    .config
                    .get("event")
                    .and_then(Value::as_str)
                    .unwrap_or("external_callback");
                let actual_event = extract_value_by_key(resume_input, "event")
                    .or_else(|| extract_value_by_key(resume_input, "type"));

                match actual_event.and_then(|value| value.as_str().map(str::to_string)) {
                    Some(actual) if actual == expected_event => {}
                    Some(actual) => {
                        return Err(RunnerError::ResumeValidation(format!(
                            "wait node {} expected event {}, got {}",
                            waiting_node.id, expected_event, actual
                        )));
                    }
                    None => {
                        return Err(RunnerError::ResumeValidation(format!(
                            "wait node {} is missing event/type in resume payload",
                            waiting_node.id
                        )));
                    }
                }

                validate_field_match(
                    waiting_node,
                    snapshot,
                    resume_input,
                    "correlationKey",
                    &["correlationKey", "requestId"],
                )?;

                Ok(())
            }
            NodeType::Task => {
                let expected_event = waiting_node
                    .config
                    .get("completeEvent")
                    .and_then(Value::as_str)
                    .unwrap_or("task.completed");
                let actual_event = extract_value_by_key(resume_input, "event")
                    .or_else(|| extract_value_by_key(resume_input, "type"));

                match actual_event.and_then(|value| value.as_str().map(str::to_string)) {
                    Some(actual) if actual == expected_event => {}
                    Some(actual) => {
                        return Err(RunnerError::ResumeValidation(format!(
                            "task node {} expected event {}, got {}",
                            waiting_node.id, expected_event, actual
                        )));
                    }
                    None => {
                        return Err(RunnerError::ResumeValidation(format!(
                            "task node {} is missing event/type in resume payload",
                            waiting_node.id
                        )));
                    }
                }

                validate_field_match(
                    waiting_node,
                    snapshot,
                    resume_input,
                    "taskId",
                    &["taskId", "id"],
                )?;

                Ok(())
            }
            NodeType::SubWorkflow => Ok(()),
            other => Err(RunnerError::ResumeValidation(format!(
                "node {} of type {} is not resumable",
                waiting_node.id,
                other.as_str()
            ))),
        }
    }

    fn resume_sub_workflow(
        &self,
        definition: &WorkflowDefinition,
        waiting_node: &crate::definition::NodeDefinition,
        snapshot: WorkflowRunSnapshot,
        resume_input: Value,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let child_snapshot = extract_child_snapshot(&snapshot)?;
        let child_definition =
            resolve_sub_workflow_definition_from_services(waiting_node, &self.services)?;
        child_definition.validate()?;

        let child_engine = WorkflowEngine::with_services(self.services.clone());
        let child_summary = child_engine.resume(&child_definition, child_snapshot, resume_input)?;
        let child_output = sub_workflow_summary_output(&child_summary);

        let mut state = snapshot.state.clone();
        if let Some(path) = waiting_node.config.get("statePath").and_then(Value::as_str) {
            merge_state(&mut state, nested_state_patch(path, child_output.clone()));
        }

        let mut timeline = snapshot.timeline.clone();
        timeline.push(NodeExecutionRecord {
            node_id: waiting_node.id.clone(),
            node_type: waiting_node.node_type,
            status: map_workflow_status_to_execution(&child_summary.status),
            output: child_output.clone(),
            state_patch: waiting_node
                .config
                .get("statePath")
                .and_then(Value::as_str)
                .map(|path| nested_state_patch(path, child_output.clone()))
                .unwrap_or(Value::Null),
            branch_key: None,
            logs: Vec::new(),
        });

        match child_summary.status {
            WorkflowRunStatus::Waiting => Ok(WorkflowRunSummary {
                run_id: snapshot.run_id.clone(),
                workflow_key: snapshot.workflow_key.clone(),
                workflow_version: snapshot.workflow_version,
                status: WorkflowRunStatus::Waiting,
                current_node_id: Some(waiting_node.id.clone()),
                state: state.clone(),
                timeline: timeline.clone(),
                last_signal: child_summary.last_signal.clone(),
                resume_state: Some(WorkflowRunSnapshot {
                    run_id: snapshot.run_id,
                    workflow_key: snapshot.workflow_key,
                    workflow_version: snapshot.workflow_version,
                    current_node_id: waiting_node.id.clone(),
                    trigger: snapshot.trigger,
                    last_input: child_output,
                    state,
                    timeline,
                    last_signal: child_summary.last_signal,
                    env: snapshot.env,
                }),
            }),
            WorkflowRunStatus::Completed => {
                let outgoing = definition.transitions_from(&waiting_node.id);
                let next = self.resolve_transition(&outgoing, None)?;
                self.execute_from(
                    definition,
                    snapshot.run_id,
                    snapshot.trigger,
                    snapshot.env,
                    state,
                    timeline,
                    next.to.clone(),
                    child_output,
                )
            }
            WorkflowRunStatus::Failed => Ok(WorkflowRunSummary {
                run_id: snapshot.run_id,
                workflow_key: snapshot.workflow_key,
                workflow_version: snapshot.workflow_version,
                status: WorkflowRunStatus::Failed,
                current_node_id: Some(waiting_node.id.clone()),
                state,
                timeline,
                last_signal: child_summary.last_signal,
                resume_state: None,
            }),
        }
    }

    fn execute_from(
        &self,
        definition: &WorkflowDefinition,
        run_id: String,
        trigger: Value,
        env: RunEnvironment,
        mut state: Value,
        mut timeline: Vec<NodeExecutionRecord>,
        mut current_node_id: String,
        mut current_input: Value,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let workflow_key = definition.meta.key.clone();
        let workflow_version = definition.meta.version;
        let max_steps = definition.nodes.len().saturating_mul(8).max(16);
        let mut last_signal = None;

        for _ in 0..max_steps {
            let node = definition
                .node(&current_node_id)
                .ok_or_else(|| RunnerError::MissingNode(current_node_id.clone()))?;
            let executor = self
                .registry
                .resolve(node.node_type)
                .ok_or_else(|| RunnerError::MissingExecutor(node.node_type.as_str().to_string()))?;
            let context = NodeExecutionContext {
                run_id: &run_id,
                workflow_key: &workflow_key,
                workflow_version,
                trigger: &trigger,
                input: &current_input,
                state: &state,
                env: &env,
            };
            let result = executor.execute(node, &context)?;
            if let Some(signal) = result.next_signal.clone() {
                last_signal = Some(signal);
            }

            if !result.state_patch.is_null() {
                merge_state(&mut state, result.state_patch.clone());
            }

            timeline.push(NodeExecutionRecord {
                node_id: node.id.clone(),
                node_type: node.node_type,
                status: result.status.clone(),
                output: result.output.clone(),
                state_patch: result.state_patch.clone(),
                branch_key: result.branch_key.clone(),
                logs: result.logs.clone(),
            });

            match result.status {
                ExecutionStatus::Waiting => {
                    let waiting_output = result.output.clone();
                    let next_signal = result.next_signal;
                    return Ok(WorkflowRunSummary {
                        run_id: run_id.clone(),
                        workflow_key,
                        workflow_version,
                        status: WorkflowRunStatus::Waiting,
                        current_node_id: Some(node.id.clone()),
                        state: state.clone(),
                        timeline: timeline.clone(),
                        last_signal: next_signal.clone().or(last_signal.clone()),
                        resume_state: Some(WorkflowRunSnapshot {
                            run_id,
                            workflow_key: definition.meta.key.clone(),
                            workflow_version: definition.meta.version,
                            current_node_id: node.id.clone(),
                            trigger: trigger.clone(),
                            last_input: waiting_output,
                            state,
                            timeline,
                            last_signal: next_signal,
                            env,
                        }),
                    });
                }
                ExecutionStatus::Failed => {
                    return Ok(WorkflowRunSummary {
                        run_id,
                        workflow_key,
                        workflow_version,
                        status: WorkflowRunStatus::Failed,
                        current_node_id: Some(node.id.clone()),
                        state,
                        timeline,
                        last_signal,
                        resume_state: None,
                    });
                }
                ExecutionStatus::Success | ExecutionStatus::Skipped => {}
            }

            if result.terminal || node.node_type == NodeType::End {
                return Ok(WorkflowRunSummary {
                    run_id,
                    workflow_key,
                    workflow_version,
                    status: WorkflowRunStatus::Completed,
                    current_node_id: Some(node.id.clone()),
                    state,
                    timeline,
                    last_signal,
                    resume_state: None,
                });
            }

            let outgoing = definition.transitions_from(&node.id);
            let next = self.resolve_transition(&outgoing, result.branch_key.as_deref())?;
            current_node_id = next.to.clone();
            current_input = result.output;
        }

        Err(RunnerError::Transition(
            "execution aborted because the max step guard was reached".to_string(),
        ))
    }
}

fn resolve_sub_workflow_definition_from_services(
    node: &crate::definition::NodeDefinition,
    services: &WorkflowServices,
) -> Result<WorkflowDefinition, RunnerError> {
    if let Some(definition) = node
        .config
        .get("definition")
        .cloned()
        .or_else(|| node.config.get("workflow").cloned())
    {
        return serde_json::from_value(definition)
            .map_err(|error| RunnerError::SubWorkflow(error.to_string()));
    }

    if let Some(reference) = node
        .config
        .get("ref")
        .and_then(Value::as_str)
        .or_else(|| node.config.get("workflowKey").and_then(Value::as_str))
    {
        return services
            .workflow_definitions
            .resolve(reference)
            .ok_or_else(|| RunnerError::MissingSubWorkflow(reference.to_string()));
    }

    Err(RunnerError::SubWorkflow(format!(
        "node {} is missing sub-workflow definition/ref",
        node.id
    )))
}

fn map_workflow_status_to_execution(status: &WorkflowRunStatus) -> ExecutionStatus {
    match status {
        WorkflowRunStatus::Completed => ExecutionStatus::Success,
        WorkflowRunStatus::Waiting => ExecutionStatus::Waiting,
        WorkflowRunStatus::Failed => ExecutionStatus::Failed,
    }
}

fn extract_child_snapshot(
    snapshot: &WorkflowRunSnapshot,
) -> Result<WorkflowRunSnapshot, RunnerError> {
    snapshot
        .last_input
        .get("resumeState")
        .cloned()
        .or_else(|| {
            snapshot
                .last_input
                .get("body")
                .and_then(|body| body.get("resumeState"))
                .cloned()
        })
        .ok_or_else(|| {
            RunnerError::SubWorkflow(format!(
                "parent run {} is missing child resumeState in sub-workflow output",
                snapshot.run_id
            ))
        })
        .and_then(|value| serde_json::from_value(value).map_err(RunnerError::Json))
}

fn sub_workflow_summary_output(summary: &WorkflowRunSummary) -> Value {
    json!({
        "workflowKey": summary.workflow_key,
        "workflowVersion": summary.workflow_version,
        "runId": summary.run_id,
        "status": summary.status,
        "state": summary.state,
        "timeline": summary.timeline,
        "lastSignal": summary.last_signal,
        "resumeState": summary.resume_state
    })
}

fn new_run_id() -> String {
    static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("run-{epoch_ms}-{sequence}")
}

fn validate_field_match(
    waiting_node: &crate::definition::NodeDefinition,
    snapshot: &WorkflowRunSnapshot,
    resume_input: &Value,
    canonical_name: &str,
    candidate_keys: &[&str],
) -> Result<(), RunnerError> {
    let expected = candidate_keys
        .iter()
        .find_map(|key| extract_value_by_key_from_signal(snapshot, key));
    let actual = candidate_keys
        .iter()
        .find_map(|key| extract_value_by_key(resume_input, key));

    match (expected, actual) {
        (Some(expected), Some(actual)) if expected == actual => Ok(()),
        (Some(expected), Some(actual)) => Err(RunnerError::ResumeValidation(format!(
            "node {} expected {} {}, got {}",
            waiting_node.id, canonical_name, expected, actual
        ))),
        (Some(_), None) => Err(RunnerError::ResumeValidation(format!(
            "node {} resume payload is missing {}",
            waiting_node.id, canonical_name
        ))),
        (None, _) => Ok(()),
    }
}

fn extract_value_by_key_from_signal(snapshot: &WorkflowRunSnapshot, key: &str) -> Option<Value> {
    snapshot
        .last_signal
        .as_ref()
        .and_then(|signal| extract_value_by_key(&signal.payload, key))
        .or_else(|| extract_value_by_key(&snapshot.last_input, key))
}

fn extract_value_by_key(value: &Value, key: &str) -> Option<Value> {
    value
        .get(key)
        .cloned()
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get(key))
                .cloned()
        })
        .or_else(|| {
            value
                .get("headers")
                .and_then(|headers| headers.get(key))
                .cloned()
        })
        .or_else(|| value.get("body").and_then(|body| body.get(key)).cloned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::definition::WorkflowDefinition;
    use crate::runtime::{RunEnvironment, WorkflowRunStatus};
    use crate::services::{FetchConnector, WorkflowServices};

    use super::WorkflowEngine;

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
                    "headers": {
                        "requestId": "req-1"
                    },
                    "body": {
                        "orderNo": "SO-1001",
                        "bizType": "manual_review"
                    }
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
                    "headers": {
                        "requestId": "req-2"
                    },
                    "body": {
                        "orderNo": "SO-1002",
                        "bizType": "auto_sort"
                    }
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
                    "headers": {
                        "requestId": "req-3"
                    },
                    "body": {
                        "orderNo": "SO-1003",
                        "bizType": "auto_sort"
                    }
                }),
                RunEnvironment::default(),
            )
            .expect("workflow run should succeed");

        let resume_state = waiting_summary
            .resume_state
            .expect("waiting run should expose resume state");
        let resumed = engine
            .resume(
                &definition,
                resume_state,
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
                    "headers": {
                        "requestId": "req-4"
                    },
                    "body": {
                        "orderNo": "SO-1004",
                        "bizType": "auto_sort"
                    }
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
                    "headers": {
                        "requestId": "req-5"
                    },
                    "body": {
                        "orderNo": "SO-1005",
                        "bizType": "manual_review"
                    }
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
                    "headers": {
                        "requestId": "req-6"
                    },
                    "body": {
                        "orderNo": "SO-1006",
                        "bizType": "auto_sort"
                    }
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
                    "headers": {
                        "requestId": "req-7"
                    },
                    "body": {
                        "orderNo": "SO-1007",
                        "bizType": "auto_sort"
                    }
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
                    "headers": {
                        "requestId": "req-coverage-1"
                    },
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
                    "headers": {
                        "requestId": "req-coverage-2"
                    },
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
    fn cascades_waiting_sub_workflow_resume_to_parent_completion() {
        let definition: WorkflowDefinition =
            serde_json::from_str(include_str!("../examples/subflow-wait-flow.json"))
                .expect("subflow wait workflow should deserialize");
        let engine = WorkflowEngine::new();
        let waiting = engine
            .run(
                &definition,
                json!({
                    "headers": {
                        "requestId": "req-sub-1"
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
                    "headers": {
                        "requestId": "req-code-1"
                    },
                    "body": {
                        "orderNo": "SO-CODE-1",
                        "qty": "6",
                        "route": "priority"
                    }
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
                    "headers": {
                        "requestId": "req-code-2"
                    },
                    "body": {
                        "orderNo": "SO-CODE-2",
                        "qty": 2,
                        "route": "normal"
                    }
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
            "meta": {
                "key": "code-timeout-flow",
                "name": "Code Timeout Flow",
                "version": 1
            },
            "trigger": {
                "type": "manual"
            },
            "inputSchema": {
                "type": "object"
            },
            "nodes": [
                {
                    "id": "start_1",
                    "type": "start",
                    "name": "Start"
                },
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
                {
                    "id": "end_1",
                    "type": "end",
                    "name": "End"
                }
            ],
            "transitions": [
                {
                    "from": "start_1",
                    "to": "run_code"
                },
                {
                    "from": "run_code",
                    "to": "end_1"
                }
            ],
            "policies": {}
        }))
        .expect("timeout workflow should deserialize");
        let engine = WorkflowEngine::new();

        let error = engine
            .run(
                &definition,
                json!({
                    "body": {
                        "orderNo": "SO-TIMEOUT-1"
                    }
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
            "meta": {
                "key": "code-source-path-flow",
                "name": "Code Source Path Flow",
                "version": 1
            },
            "trigger": {
                "type": "manual"
            },
            "inputSchema": {
                "type": "object"
            },
            "nodes": [
                {
                    "id": "start_1",
                    "type": "start",
                    "name": "Start"
                },
                {
                    "id": "run_code",
                    "type": "code",
                    "name": "Run Code",
                    "inputMapping": {
                        "orderNo": "{{input.orderNo}}"
                    },
                    "config": {
                        "language": "js",
                        "sourcePath": "examples/code-source-handler.js"
                    }
                },
                {
                    "id": "end_1",
                    "type": "end",
                    "name": "End"
                }
            ],
            "transitions": [
                {
                    "from": "start_1",
                    "to": "run_code"
                },
                {
                    "from": "run_code",
                    "to": "end_1"
                }
            ],
            "policies": {}
        }))
        .expect("source path workflow should deserialize");
        let engine = WorkflowEngine::new();
        let summary = engine
            .run(
                &definition,
                json!({
                    "body": {
                        "orderNo": "SO-SOURCE-1"
                    }
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
}
