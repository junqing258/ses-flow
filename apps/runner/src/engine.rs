use std::sync::Arc;
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
use crate::template::merge_state;

pub struct WorkflowEngine {
    registry: ExecutorRegistry,
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self::with_services(WorkflowServices::with_defaults())
    }

    pub fn with_services(services: WorkflowServices) -> Self {
        Self {
            registry: ExecutorRegistry::with_defaults(Arc::new(services)),
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
                        last_signal: next_signal.clone(),
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
                        last_signal: result.next_signal,
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
                    last_signal: result.next_signal,
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

fn new_run_id() -> String {
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("run-{epoch_ms}")
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
}
