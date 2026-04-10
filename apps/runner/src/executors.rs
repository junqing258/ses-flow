use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::definition::{NodeDefinition, NodeType, WorkflowDefinition};
use crate::error::RunnerError;
use crate::runtime::{
    NextSignal, NodeExecutionContext, NodeExecutionResult, NodeLogRecord, RunEnvironment,
    WorkflowRunStatus,
};
use crate::services::WorkflowServices;
use crate::template::{EvaluationContext, env_to_value, is_truthy, nested_state_patch};

pub trait NodeExecutor: Send + Sync {
    fn node_type(&self) -> NodeType;
    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError>;
}

#[derive(Default)]
pub struct ExecutorRegistry {
    executors: HashMap<NodeType, Arc<dyn NodeExecutor>>,
}

impl ExecutorRegistry {
    pub fn with_defaults(services: Arc<WorkflowServices>) -> Self {
        let mut registry = Self::default();
        registry.register(StartExecutor);
        registry.register(EndExecutor);
        registry.register(WebhookTriggerExecutor);
        registry.register(FetchExecutor {
            services: services.clone(),
        });
        registry.register(SetStateExecutor);
        registry.register(IfElseExecutor);
        registry.register(SwitchExecutor);
        registry.register(CodeExecutor);
        registry.register(ActionExecutor {
            services: services.clone(),
        });
        registry.register(RespondExecutor);
        registry.register(WaitExecutor);
        registry.register(TaskExecutor {
            services: services.clone(),
        });
        registry.register(SubWorkflowExecutor { services });
        registry
    }

    pub fn register<E>(&mut self, executor: E)
    where
        E: NodeExecutor + 'static,
    {
        self.executors
            .insert(executor.node_type(), Arc::new(executor));
    }

    pub fn resolve(&self, node_type: NodeType) -> Option<Arc<dyn NodeExecutor>> {
        self.executors.get(&node_type).cloned()
    }
}

struct StartExecutor;
struct EndExecutor;
struct WebhookTriggerExecutor;
struct FetchExecutor {
    services: Arc<WorkflowServices>,
}
struct SetStateExecutor;
struct IfElseExecutor;
struct SwitchExecutor;
struct CodeExecutor;
struct ActionExecutor {
    services: Arc<WorkflowServices>,
}
struct RespondExecutor;
struct WaitExecutor;
struct TaskExecutor {
    services: Arc<WorkflowServices>,
}
struct SubWorkflowExecutor {
    services: Arc<WorkflowServices>,
}

impl NodeExecutor for StartExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Start
    }

    fn execute(
        &self,
        _node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let output = context
            .trigger
            .get("body")
            .cloned()
            .unwrap_or_else(|| context.trigger.clone());
        Ok(NodeExecutionResult::success(output))
    }
}

impl NodeExecutor for EndExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::End
    }

    fn execute(
        &self,
        _node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        Ok(NodeExecutionResult::success(context.input.clone()).into_terminal())
    }
}

impl NodeExecutor for WebhookTriggerExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::WebhookTrigger
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let mode = node
            .config
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("body");
        let payload = match mode {
            "full" => context.trigger.clone(),
            "headers" => context
                .trigger
                .get("headers")
                .cloned()
                .unwrap_or(Value::Null),
            _ => context
                .trigger
                .get("body")
                .cloned()
                .unwrap_or_else(|| context.trigger.clone()),
        };

        Ok(NodeExecutionResult::success(payload))
    }
}

impl NodeExecutor for FetchExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Fetch
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let request = resolve_mapping(node, context);
        let connector = node
            .config
            .get("connector")
            .and_then(Value::as_str)
            .unwrap_or("connector.unknown");
        let connector_impl = self
            .services
            .fetch_connectors
            .resolve(connector)
            .ok_or_else(|| RunnerError::MissingFetchConnector(connector.to_string()))?;
        let data = connector_impl.fetch(&request, context)?;

        Ok(NodeExecutionResult::success(json!({
            "connector": connector,
            "request": request,
            "data": data
        })))
    }
}

impl NodeExecutor for SetStateExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::SetState
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let state_path = node
            .config
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("statePatch");
        let state_value = payload.get("value").cloned().unwrap_or(payload.clone());

        Ok(NodeExecutionResult::success(state_value.clone())
            .with_state_patch(nested_state_patch(state_path, state_value)))
    }
}

impl NodeExecutor for IfElseExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::IfElse
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let template_context = evaluation_context(context, &Value::Null);
        let expression = node
            .config
            .get("expression")
            .cloned()
            .unwrap_or_else(|| Value::Bool(false));
        let resolved = template_context.resolve_value(&expression);
        let expected = node
            .config
            .get("equals")
            .map(|value| template_context.resolve_value(value));
        let condition = match expected {
            Some(expected) => resolved == expected,
            None => is_truthy(&resolved),
        };
        let branch_key = if condition { "then" } else { "else" };

        Ok(NodeExecutionResult::success(json!({
            "condition": condition,
            "value": resolved
        }))
        .with_branch_key(branch_key))
    }
}

impl NodeExecutor for SwitchExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Switch
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let template_context = EvaluationContext {
            trigger: context.trigger,
            input: context.input,
            state: context.state,
            env: env_to_value(context.env),
            output: &Value::Null,
        };

        let expression = node
            .config
            .get("expression")
            .cloned()
            .unwrap_or_else(|| Value::String("default".to_string()));
        let branch_value = template_context.resolve_value(&expression);
        let branch_key = match branch_value {
            Value::String(value) => value,
            Value::Null => "default".to_string(),
            other => other.to_string(),
        };

        Ok(
            NodeExecutionResult::success(json!({ "branch": branch_key }))
                .with_branch_key(branch_key),
        )
    }
}

impl NodeExecutor for ActionExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Action
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let action = node
            .config
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("action.unknown");
        let handler = self
            .services
            .action_handlers
            .resolve(action)
            .ok_or_else(|| RunnerError::MissingActionHandler(action.to_string()))?;
        let response = handler.execute(&payload, context)?;

        Ok(NodeExecutionResult::success(json!({
            "action": action,
            "response": response
        })))
    }
}

impl NodeExecutor for CodeExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Code
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let language = node
            .config
            .get("language")
            .or_else(|| node.config.get("lang"))
            .and_then(Value::as_str)
            .unwrap_or("js");
        if !matches!(language, "js" | "javascript") {
            return Err(RunnerError::CodeExecution(format!(
                "node {} only supports js/javascript, got {}",
                node.id, language
            )));
        }

        let params = resolve_mapping(node, context);
        let spec = resolve_code_execution_spec(node)?;
        let process_output = execute_js_code(&spec, context, &params, node.timeout_ms)?;

        Ok(build_code_result(process_output.result).with_logs(process_output.logs))
    }
}

impl NodeExecutor for RespondExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Respond
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let status_code = node
            .config
            .get("statusCode")
            .and_then(Value::as_u64)
            .unwrap_or(200);
        let response = json!({
            "statusCode": status_code,
            "body": payload
        });

        let mut result = NodeExecutionResult::success(payload).with_signal(NextSignal {
            signal_type: "webhook_response".to_string(),
            payload: response,
        });

        if node
            .config
            .get("terminal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            result = result.into_terminal();
        }

        Ok(result)
    }
}

impl NodeExecutor for WaitExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Wait
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let event = node
            .config
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("external_callback");

        Ok(NodeExecutionResult::waiting(
            NextSignal {
                signal_type: event.to_string(),
                payload: payload.clone(),
            },
            json!({
                "event": event,
                "payload": payload
            }),
        ))
    }
}

impl NodeExecutor for TaskExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Task
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let task_type = node
            .config
            .get("taskType")
            .and_then(Value::as_str)
            .unwrap_or("generic_task");
        let handler = self
            .services
            .task_handlers
            .resolve(task_type)
            .ok_or_else(|| RunnerError::MissingTaskHandler(task_type.to_string()))?;
        let task = handler.create(&payload, context)?;

        Ok(NodeExecutionResult::waiting(
            NextSignal {
                signal_type: "task_created".to_string(),
                payload: task.clone(),
            },
            json!({
                "taskType": task_type,
                "task": task
            }),
        ))
    }
}

impl NodeExecutor for SubWorkflowExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::SubWorkflow
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let input = resolve_mapping(node, context);
        let definition = resolve_sub_workflow_definition(node, &self.services)?;
        definition.validate()?;

        let engine = crate::engine::WorkflowEngine::with_services((*self.services).clone());
        let nested_trigger = json!({
            "headers": {
                "parentRunId": context.run_id,
                "parentWorkflowKey": context.workflow_key,
                "parentNodeId": node.id
            },
            "body": input
        });
        let summary = engine.run(&definition, nested_trigger, clone_env(context.env))?;

        let output = json!({
            "workflowKey": summary.workflow_key,
            "workflowVersion": summary.workflow_version,
            "runId": summary.run_id,
            "status": summary.status,
            "state": summary.state,
            "timeline": summary.timeline,
            "lastSignal": summary.last_signal,
            "resumeState": summary.resume_state
        });

        let export_path = node.config.get("statePath").and_then(Value::as_str);

        match summary.status {
            WorkflowRunStatus::Completed => {
                let mut result = NodeExecutionResult::success(output.clone());
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
            WorkflowRunStatus::Failed => {
                let mut result = NodeExecutionResult::failed(
                    "sub_workflow_failed",
                    format!("sub-workflow {} failed", definition.meta.key),
                    false,
                );
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
            WorkflowRunStatus::Waiting => {
                let nested_signal = summary.last_signal.clone().unwrap_or(NextSignal {
                    signal_type: "sub_workflow_waiting".to_string(),
                    payload: json!({
                        "childWorkflowKey": summary.workflow_key,
                        "childRunId": summary.run_id
                    }),
                });
                let waiting_output = output.clone();
                let mut result = NodeExecutionResult::waiting(nested_signal, output);
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, waiting_output));
                }
                Ok(result)
            }
        }
    }
}

fn resolve_mapping(node: &NodeDefinition, context: &NodeExecutionContext<'_>) -> Value {
    let template_context = evaluation_context(context, &Value::Null);

    if node.input_mapping.is_null() {
        return context.input.clone();
    }

    template_context.resolve_value(&node.input_mapping)
}

fn resolve_code_execution_spec(node: &NodeDefinition) -> Result<CodeExecutionSpec, RunnerError> {
    if let Some(source) = node
        .config
        .get("source")
        .or_else(|| node.config.get("js"))
        .or_else(|| node.config.get("code"))
        .and_then(Value::as_str)
    {
        return Ok(CodeExecutionSpec::InlineSource(source.to_string()));
    }

    if let Some(source_path) = node
        .config
        .get("sourcePath")
        .or_else(|| node.config.get("filePath"))
        .and_then(Value::as_str)
    {
        let resolved = resolve_code_file_path(source_path)?;
        let source = fs::read_to_string(&resolved).map_err(|error| {
            RunnerError::CodeExecution(format!(
                "failed to read code source file {}: {error}",
                resolved.display()
            ))
        })?;
        return Ok(CodeExecutionSpec::InlineSource(source));
    }

    if let Some(module_path) = node.config.get("modulePath").and_then(Value::as_str) {
        let resolved = resolve_code_file_path(module_path)?;
        let export_name = node
            .config
            .get("exportName")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        return Ok(CodeExecutionSpec::Module {
            module_path: resolved.to_string_lossy().to_string(),
            export_name,
        });
    }

    Err(RunnerError::CodeExecution(format!(
        "node {} is missing config.source/js/code, sourcePath/filePath, or modulePath",
        node.id
    )))
}

fn resolve_code_file_path(path: &str) -> Result<PathBuf, RunnerError> {
    let file_path = Path::new(path);
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| RunnerError::CodeExecution(error.to_string()))?
            .join(file_path)
    };

    absolute.canonicalize().map_err(|error| {
        RunnerError::CodeExecution(format!(
            "failed to resolve code file path {}: {error}",
            absolute.display()
        ))
    })
}

fn execute_js_code(
    spec: &CodeExecutionSpec,
    context: &NodeExecutionContext<'_>,
    params: &Value,
    timeout_ms: Option<u64>,
) -> Result<CodeProcessOutput, RunnerError> {
    let mut payload = json!({
        "trigger": context.trigger,
        "input": context.input,
        "state": context.state,
        "env": env_to_value(context.env),
        "params": params
    });
    match spec {
        CodeExecutionSpec::InlineSource(source) => {
            payload["source"] = Value::String(source.clone());
        }
        CodeExecutionSpec::Module {
            module_path,
            export_name,
        } => {
            payload["modulePath"] = Value::String(module_path.clone());
            payload["exportName"] = Value::String(export_name.clone());
        }
    }
    let request = serde_json::to_vec(&payload)
        .map_err(|error| RunnerError::CodeExecution(error.to_string()))?;

    let mut child = Command::new("node")
        .args(["--input-type=module", "--eval", NODE_RUNNER_BOOTSTRAP])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| RunnerError::CodeExecution(error.to_string()))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RunnerError::CodeExecution("failed to open node stdin".to_string()))?;
        stdin
            .write_all(&request)
            .map_err(|error| RunnerError::CodeExecution(error.to_string()))?;
    }

    let output = wait_for_code_process(child, timeout_ms)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RunnerError::CodeExecution(if stderr.is_empty() {
            format!("node process exited with status {}", output.status)
        } else {
            stderr
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(CodeProcessOutput {
            result: Value::Null,
            logs: Vec::new(),
        });
    }

    serde_json::from_str::<CodeProcessOutput>(&stdout).map_err(|error| {
        RunnerError::CodeExecution(format!("code node returned non-JSON value: {error}"))
    })
}

fn build_code_result(result: Value) -> NodeExecutionResult {
    let Value::Object(mut object) = result.clone() else {
        return NodeExecutionResult::success(result);
    };

    let envelope = object.contains_key("output")
        || object.contains_key("statePatch")
        || object.contains_key("state_patch")
        || object.contains_key("state")
        || object.contains_key("branchKey")
        || object.contains_key("branch_key");
    if !envelope {
        return NodeExecutionResult::success(result);
    }

    let output = object.remove("output").unwrap_or(Value::Null);
    let state_patch = object
        .remove("statePatch")
        .or_else(|| object.remove("state_patch"))
        .or_else(|| object.remove("state"))
        .unwrap_or(Value::Null);
    let branch_key = object
        .remove("branchKey")
        .or_else(|| object.remove("branch_key"))
        .and_then(value_to_branch_key);

    let mut execution = NodeExecutionResult::success(output).with_state_patch(state_patch);
    if let Some(branch_key) = branch_key {
        execution = execution.with_branch_key(branch_key);
    }
    execution
}

fn value_to_branch_key(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value),
        other => Some(other.to_string()),
    }
}

fn wait_for_code_process(
    mut child: std::process::Child,
    timeout_ms: Option<u64>,
) -> Result<std::process::Output, RunnerError> {
    let Some(timeout_ms) = timeout_ms else {
        return child
            .wait_with_output()
            .map_err(|error| RunnerError::CodeExecution(error.to_string()));
    };

    let started_at = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        match child
            .try_wait()
            .map_err(|error| RunnerError::CodeExecution(error.to_string()))?
        {
            Some(_) => {
                return child
                    .wait_with_output()
                    .map_err(|error| RunnerError::CodeExecution(error.to_string()));
            }
            None if started_at.elapsed() >= timeout => {
                child
                    .kill()
                    .map_err(|error| RunnerError::CodeExecution(error.to_string()))?;
                let _ = child.wait();
                return Err(RunnerError::CodeExecution(format!(
                    "code node exceeded timeout of {timeout_ms}ms"
                )));
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

const NODE_RUNNER_BOOTSTRAP: &str = r#"
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const chunks = [];
for await (const chunk of process.stdin) {
  chunks.push(chunk);
}

const raw = Buffer.concat(chunks).toString('utf8').trim();
const payload = raw ? JSON.parse(raw) : {};
const {
  source = '',
  modulePath = null,
  exportName = 'default',
  trigger = null,
  input = null,
  state = null,
  env = null,
  params = null
} = payload;
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const majorVersion = Number((process.versions.node || '0').split('.')[0] || '0');
if (!Number.isFinite(majorVersion) || majorVersion < 22) {
  throw new Error(`Code node requires Node.js 22+, got ${process.version}`);
}
const logs = [];
const normalize = (value) => {
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
};
const patchConsole = (level) => (...args) => {
  logs.push({
    level,
    message: args.map(normalize).join(' ')
  });
};
globalThis.console = {
  ...console,
  log: patchConsole('log'),
  info: patchConsole('info'),
  warn: patchConsole('warn'),
  error: patchConsole('error'),
  debug: patchConsole('debug')
};

try {
  let result;
  if (modulePath) {
    const module = await import(pathToFileURL(modulePath).href);
    const handler = exportName === 'default' ? module.default : module[exportName];
    if (typeof handler !== 'function') {
      throw new Error(`Code module export "${exportName}" is not a function`);
    }
    result = await handler(trigger, input, state, env, params);
  } else {
    const fn = new AsyncFunction('trigger', 'input', 'state', 'env', 'params', source);
    result = await fn(trigger, input, state, env, params);
  }
  process.stdout.write(JSON.stringify({
    result: result === undefined ? null : result,
    logs
  }));
} catch (error) {
  const message = error && typeof error.stack === 'string' ? error.stack : String(error);
  process.stderr.write(message);
  process.exit(1);
}
"#;

#[derive(serde::Deserialize)]
struct CodeProcessOutput {
    result: Value,
    #[serde(default)]
    logs: Vec<NodeLogRecord>,
}

enum CodeExecutionSpec {
    InlineSource(String),
    Module {
        module_path: String,
        export_name: String,
    },
}

fn evaluation_context<'a>(
    context: &'a NodeExecutionContext<'a>,
    output: &'a Value,
) -> EvaluationContext<'a> {
    let env = env_to_value(context.env);
    EvaluationContext {
        trigger: context.trigger,
        input: context.input,
        state: context.state,
        env,
        output,
    }
}

fn resolve_sub_workflow_definition(
    node: &NodeDefinition,
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

fn clone_env(env: &RunEnvironment) -> RunEnvironment {
    RunEnvironment {
        tenant_id: env.tenant_id.clone(),
        warehouse_id: env.warehouse_id.clone(),
        operator_id: env.operator_id.clone(),
    }
}
