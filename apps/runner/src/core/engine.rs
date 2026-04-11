use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tracing::{debug, error, info, warn};

use super::definition::{NodeType, TransitionDefinition, WorkflowDefinition};
use super::executors::ExecutorRegistry;
use super::runtime::{
    ExecutionStatus, NodeExecutionContext, NodeExecutionRecord, NoopWorkflowRunController,
    NoopWorkflowRunObserver, RunEnvironment, WorkflowRunController, WorkflowRunObserver,
    WorkflowRunSnapshot, WorkflowRunStatus, WorkflowRunSummary,
};
use super::template::{merge_state, nested_state_patch};
use crate::error::RunnerError;
use crate::services::WorkflowServices;

pub struct WorkflowEngine {
    registry: ExecutorRegistry,
    services: WorkflowServices,
    observer: Arc<dyn WorkflowRunObserver>,
    controller: Arc<dyn WorkflowRunController>,
}

impl WorkflowEngine {
    pub fn new() -> Self {
        Self::with_services(WorkflowServices::with_defaults())
    }

    pub fn with_services(services: WorkflowServices) -> Self {
        Self::with_services_observer_and_controller(
            services,
            Arc::new(NoopWorkflowRunObserver),
            Arc::new(NoopWorkflowRunController),
        )
    }

    pub fn with_observer(observer: Arc<dyn WorkflowRunObserver>) -> Self {
        Self::with_services_observer_and_controller(
            WorkflowServices::with_defaults(),
            observer,
            Arc::new(NoopWorkflowRunController),
        )
    }

    pub fn with_services_and_observer(
        services: WorkflowServices,
        observer: Arc<dyn WorkflowRunObserver>,
    ) -> Self {
        Self::with_services_observer_and_controller(
            services,
            observer,
            Arc::new(NoopWorkflowRunController),
        )
    }

    pub fn with_services_observer_and_controller(
        services: WorkflowServices,
        observer: Arc<dyn WorkflowRunObserver>,
        controller: Arc<dyn WorkflowRunController>,
    ) -> Self {
        Self {
            registry: ExecutorRegistry::with_defaults(Arc::new(services.clone())),
            services,
            observer,
            controller,
        }
    }

    pub fn run(
        &self,
        definition: &WorkflowDefinition,
        trigger: Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        self.run_with_id(definition, new_run_id(), trigger, env)
    }

    pub fn run_with_id(
        &self,
        definition: &WorkflowDefinition,
        run_id: String,
        trigger: Value,
        env: RunEnvironment,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        info!(
            run_id = %run_id,
            workflow_key = definition.meta.key,
            workflow_version = definition.meta.version,
            "starting workflow execution",
        );
        definition.validate()?;
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
        info!(
            run_id = %snapshot.run_id,
            workflow_key = definition.meta.key,
            workflow_version = definition.meta.version,
            current_node_id = %snapshot.current_node_id,
            "resuming workflow execution",
        );
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
        let mut timeline = snapshot.timeline.clone();
        timeline.push(NodeExecutionRecord {
            node_id: waiting_node.id.clone(),
            node_type: waiting_node.node_type,
            status: ExecutionStatus::Success,
            output: resume_input.clone(),
            state_patch: Value::Null,
            branch_key: None,
            logs: Vec::new(),
        });

        self.execute_from(
            definition,
            snapshot.run_id,
            snapshot.trigger,
            snapshot.env,
            snapshot.state,
            timeline,
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

    fn describe_transitions(transitions: &[&TransitionDefinition]) -> String {
        transitions
            .iter()
            .map(|transition| {
                format!(
                    "{}->{}(label={}, branch_type={}, condition={})",
                    transition.from,
                    transition.to,
                    transition.label.as_deref().unwrap_or(""),
                    transition.branch_type.as_deref().unwrap_or(""),
                    transition.condition.as_deref().unwrap_or(""),
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn validate_resume_input(
        &self,
        waiting_node: &super::definition::NodeDefinition,
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
        waiting_node: &super::definition::NodeDefinition,
        snapshot: WorkflowRunSnapshot,
        resume_input: Value,
    ) -> Result<WorkflowRunSummary, RunnerError> {
        let child_snapshot = extract_child_snapshot(&snapshot)?;
        let child_definition =
            resolve_sub_workflow_definition_from_services(waiting_node, &self.services)?;
        child_definition.validate()?;

        let child_engine = WorkflowEngine::with_services_observer_and_controller(
            self.services.clone(),
            Arc::new(NoopWorkflowRunObserver),
            self.controller.clone(),
        );
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
            WorkflowRunStatus::Running => Err(RunnerError::SubWorkflow(format!(
                "sub-workflow {} returned unexpected running status",
                child_summary.workflow_key
            ))),
            WorkflowRunStatus::Waiting => {
                let summary = WorkflowRunSummary {
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
                };
                self.emit_summary(&summary);
                Ok(summary)
            }
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
            WorkflowRunStatus::Failed => {
                let summary = WorkflowRunSummary {
                    run_id: snapshot.run_id,
                    workflow_key: snapshot.workflow_key,
                    workflow_version: snapshot.workflow_version,
                    status: WorkflowRunStatus::Failed,
                    current_node_id: Some(waiting_node.id.clone()),
                    state,
                    timeline,
                    last_signal: child_summary.last_signal,
                    resume_state: None,
                };
                self.emit_summary(&summary);
                Ok(summary)
            }
            WorkflowRunStatus::Terminated => {
                let summary = WorkflowRunSummary {
                    run_id: snapshot.run_id,
                    workflow_key: snapshot.workflow_key,
                    workflow_version: snapshot.workflow_version,
                    status: WorkflowRunStatus::Terminated,
                    current_node_id: Some(waiting_node.id.clone()),
                    state,
                    timeline,
                    last_signal: child_summary.last_signal,
                    resume_state: None,
                };
                self.emit_summary(&summary);
                Ok(summary)
            }
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
        info!(
            run_id = %run_id,
            workflow_key = %workflow_key,
            workflow_version,
            current_node_id = %current_node_id,
            max_steps,
            "entering workflow execution loop",
        );

        self.emit_summary(&WorkflowRunSummary {
            run_id: run_id.clone(),
            workflow_key: workflow_key.clone(),
            workflow_version,
            status: WorkflowRunStatus::Running,
            current_node_id: Some(current_node_id.clone()),
            state: state.clone(),
            timeline: timeline.clone(),
            last_signal: None,
            resume_state: None,
        });

        for _ in 0..max_steps {
            if self.controller.should_terminate(&run_id) {
                let summary = self.terminated_summary(
                    &run_id,
                    &workflow_key,
                    workflow_version,
                    Some(current_node_id.clone()),
                    state,
                    timeline,
                    last_signal,
                );
                self.emit_summary(&summary);
                return Ok(summary);
            }

            let node = definition
                .node(&current_node_id)
                .ok_or_else(|| RunnerError::MissingNode(current_node_id.clone()))?;
            debug!(
                run_id = %run_id,
                workflow_key = %workflow_key,
                node_id = %node.id,
                node_type = node.node_type.as_str(),
                "executing workflow node",
            );
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
            info!(
                run_id = %run_id,
                workflow_key = %workflow_key,
                node_id = %node.id,
                node_type = node.node_type.as_str(),
                input = %serde_json::to_string(&current_input).unwrap_or_else(|_| "serialize error".to_string()),
                "node input before execution",
            );
            let result = executor.execute(node, &context)?;
            info!(
                run_id = %run_id,
                workflow_key = %workflow_key,
                node_id = %node.id,
                node_type = node.node_type.as_str(),
                output = %serde_json::to_string(&result.output).unwrap_or_else(|_| "serialize error".to_string()),
                execution_status = ?result.status,
                branch_key = result.branch_key.as_deref().unwrap_or(""),
                terminal = result.terminal,
                log_count = result.logs.len(),
                "node output after execution",
            );
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

            if self.controller.should_terminate(&run_id) {
                let summary = self.terminated_summary(
                    &run_id,
                    &workflow_key,
                    workflow_version,
                    Some(node.id.clone()),
                    state,
                    timeline,
                    last_signal,
                );
                self.emit_summary(&summary);
                return Ok(summary);
            }

            match result.status {
                ExecutionStatus::Waiting => {
                    info!(
                        run_id = %run_id,
                        workflow_key = %workflow_key,
                        node_id = %node.id,
                        "workflow execution paused and is waiting for resume event",
                    );
                    let waiting_output = result.output.clone();
                    let next_signal = result.next_signal;
                    let summary = WorkflowRunSummary {
                        run_id: run_id.clone(),
                        workflow_key: workflow_key.clone(),
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
                    };
                    self.emit_summary(&summary);
                    return Ok(summary);
                }
                ExecutionStatus::Failed => {
                    warn!(
                        run_id = %run_id,
                        workflow_key = %workflow_key,
                        node_id = %node.id,
                        "workflow execution entered failed state",
                    );
                    let summary = WorkflowRunSummary {
                        run_id,
                        workflow_key,
                        workflow_version,
                        status: WorkflowRunStatus::Failed,
                        current_node_id: Some(node.id.clone()),
                        state,
                        timeline,
                        last_signal,
                        resume_state: None,
                    };
                    self.emit_summary(&summary);
                    return Ok(summary);
                }
                ExecutionStatus::Success | ExecutionStatus::Skipped => {}
            }

            if result.terminal || node.node_type == NodeType::End {
                info!(
                    run_id = %run_id,
                    workflow_key = %workflow_key,
                    node_id = %node.id,
                    "workflow execution completed",
                );
                let summary = WorkflowRunSummary {
                    run_id,
                    workflow_key,
                    workflow_version,
                    status: WorkflowRunStatus::Completed,
                    current_node_id: Some(node.id.clone()),
                    state,
                    timeline,
                    last_signal,
                    resume_state: None,
                };
                self.emit_summary(&summary);
                return Ok(summary);
            }

            let outgoing = definition.transitions_from(&node.id);
            let next = self
                .resolve_transition(&outgoing, result.branch_key.as_deref())
                .map_err(|error| match error {
                    RunnerError::Transition(message) => RunnerError::Transition(format!(
                        "{}; node_id={}; outgoing=[{}]",
                        message,
                        node.id,
                        Self::describe_transitions(&outgoing),
                    )),
                    other => other,
                })?;
            debug!(
                run_id = %run_id,
                workflow_key = %workflow_key,
                from_node_id = %node.id,
                to_node_id = %next.to,
                branch_key = result.branch_key.as_deref().unwrap_or(""),
                "resolved next workflow transition",
            );
            current_node_id = next.to.clone();
            current_input = result.output;

            self.emit_summary(&WorkflowRunSummary {
                run_id: run_id.clone(),
                workflow_key: workflow_key.clone(),
                workflow_version,
                status: WorkflowRunStatus::Running,
                current_node_id: Some(current_node_id.clone()),
                state: state.clone(),
                timeline: timeline.clone(),
                last_signal: last_signal.clone(),
                resume_state: None,
            });
        }

        error!(
            run_id = %run_id,
            workflow_key = %workflow_key,
            max_steps,
            "workflow execution aborted after reaching the max step guard",
        );
        Err(RunnerError::Transition(
            "execution aborted because the max step guard was reached".to_string(),
        ))
    }

    fn emit_summary(&self, summary: &WorkflowRunSummary) {
        self.observer.on_summary(summary);
    }

    fn terminated_summary(
        &self,
        run_id: &str,
        workflow_key: &str,
        workflow_version: u32,
        current_node_id: Option<String>,
        state: Value,
        timeline: Vec<NodeExecutionRecord>,
        last_signal: Option<super::runtime::NextSignal>,
    ) -> WorkflowRunSummary {
        WorkflowRunSummary {
            run_id: run_id.to_string(),
            workflow_key: workflow_key.to_string(),
            workflow_version,
            status: WorkflowRunStatus::Terminated,
            current_node_id,
            state,
            timeline,
            last_signal,
            resume_state: None,
        }
    }
}

fn resolve_sub_workflow_definition_from_services(
    node: &super::definition::NodeDefinition,
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
        WorkflowRunStatus::Running => ExecutionStatus::Success,
        WorkflowRunStatus::Completed => ExecutionStatus::Success,
        WorkflowRunStatus::Waiting => ExecutionStatus::Waiting,
        WorkflowRunStatus::Failed => ExecutionStatus::Failed,
        WorkflowRunStatus::Terminated => ExecutionStatus::Failed,
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

pub fn new_run_id() -> String {
    static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("run-{epoch_ms}-{sequence}")
}

fn validate_field_match(
    waiting_node: &super::definition::NodeDefinition,
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
