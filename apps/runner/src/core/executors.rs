use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use reqwest::Method;
use reqwest::blocking::{Client, RequestBuilder, Response};
use serde_json::{Value, json};

use super::definition::{
    NodeDefinition, NodeType, WorkflowDefinition, deserialize_workflow_definition,
};
use super::runtime::{
    NextSignal, NodeExecutionContext, NodeExecutionResult, NodeLogRecord, RunEnvironment,
    WorkflowRunStatus,
};
use super::template::{EvaluationContext, env_to_value, is_truthy, nested_state_patch};
use crate::error::RunnerError;
use crate::services::WorkflowServices;

// region 执行器抽象与注册表
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
        registry.register(FetchExecutor);
        registry.register(SetStateExecutor);
        registry.register(IfElseExecutor);
        registry.register(SwitchExecutor);
        registry.register(CodeExecutor);
        registry.register(ShellExecutor);
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
// endregion 执行器抽象与注册表

// region 执行器类型声明
struct StartExecutor;
struct EndExecutor;
struct WebhookTriggerExecutor;
struct FetchExecutor;
struct SetStateExecutor;
struct IfElseExecutor;
struct SwitchExecutor;
struct CodeExecutor;
struct ShellExecutor;
struct RespondExecutor;
struct WaitExecutor;
struct TaskExecutor {
    services: Arc<WorkflowServices>,
}
struct SubWorkflowExecutor {
    services: Arc<WorkflowServices>,
}
// endregion 执行器类型声明

// region 起止节点执行器
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
// endregion 起止节点执行器

// region 触发与网络节点执行器
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
        let resolved_config = resolve_config(node, context, &request);
        let method = resolve_fetch_method(&resolved_config)?;
        let url = resolve_fetch_url(&resolved_config)?;
        let headers = resolve_fetch_headers(&resolved_config)?;
        let client = build_fetch_client(node.timeout_ms)?;
        let request_builder = build_fetch_request(&client, &method, &url, headers, &request)?;
        let response = request_builder
            .send()
            .map_err(|error| RunnerError::FetchRequest(error.to_string()))?;
        let response_payload = build_fetch_response_payload(response)?;

        Ok(NodeExecutionResult::success(json!({
            "method": method.as_str(),
            "url": response_payload["url"],
            "request": request,
            "response": response_payload["response"],
            "data": response_payload["data"]
        })))
    }
}
// endregion 触发与网络节点执行器

// region 状态与分支节点执行器
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
        let evaluated = template_context.resolve_value(&expression);
        let branch_key = if is_truthy(&evaluated) {
            "then"
        } else {
            "else"
        };

        Ok(NodeExecutionResult::success(json!({
            "branch": branch_key,
            "matched": branch_key == "then"
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
// endregion 状态与分支节点执行器

// region 动作与代码节点执行器
impl NodeExecutor for ShellExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Shell
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let payload = resolve_mapping(node, context);
        let resolved_config = resolve_config(node, context, &payload);
        let command = resolved_config
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                RunnerError::InvalidShellConfig(format!(
                    "node {} is missing config.command",
                    node.id
                ))
            })?;
        let spec = resolve_shell_execution_spec(&resolved_config, command)?;
        let output = execute_shell_command(&spec, context, &payload, node.timeout_ms)?;

        Ok(NodeExecutionResult::success(output.result).with_logs(output.logs))
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
// endregion 动作与代码节点执行器

// region 响应与等待节点执行器
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
// endregion 响应与等待节点执行器

// region 任务与子流程节点执行器
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

        let engine = super::WorkflowEngine::with_services((*self.services).clone());
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
            WorkflowRunStatus::Running => Err(RunnerError::SubWorkflow(format!(
                "sub-workflow {} returned unexpected running status",
                definition.meta.key
            ))),
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
            WorkflowRunStatus::Terminated => {
                let mut result = NodeExecutionResult::failed(
                    "sub_workflow_terminated",
                    format!("sub-workflow {} was terminated", definition.meta.key),
                    false,
                );
                if let Some(path) = export_path {
                    result = result.with_state_patch(nested_state_patch(path, output));
                }
                Ok(result)
            }
        }
    }
}
// endregion 任务与子流程节点执行器

// region 通用解析辅助函数
fn resolve_mapping(node: &NodeDefinition, context: &NodeExecutionContext<'_>) -> Value {
    let template_context = evaluation_context(context, &Value::Null);

    if node.input_mapping.is_null() {
        return context.input.clone();
    }

    template_context.resolve_value(&node.input_mapping)
}

fn resolve_config(
    node: &NodeDefinition,
    context: &NodeExecutionContext<'_>,
    output: &Value,
) -> Value {
    evaluation_context(context, output).resolve_value(&node.config)
}
// endregion 通用解析辅助函数

// region Fetch 节点辅助函数
fn resolve_fetch_method(config: &Value) -> Result<Method, RunnerError> {
    let method = config
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .trim()
        .to_ascii_uppercase();

    method.parse::<Method>().map_err(|error| {
        RunnerError::InvalidFetchConfig(format!("unsupported fetch method {method}: {error}"))
    })
}

fn resolve_fetch_url(config: &Value) -> Result<String, RunnerError> {
    let Some(url) = config.get("url").and_then(Value::as_str).map(str::trim) else {
        return Err(RunnerError::InvalidFetchConfig(
            "fetch node config.url is required".to_string(),
        ));
    };

    if url.is_empty() {
        return Err(RunnerError::InvalidFetchConfig(
            "fetch node config.url cannot be empty".to_string(),
        ));
    }

    Ok(url.to_string())
}

fn resolve_fetch_headers(config: &Value) -> Result<Vec<(String, String)>, RunnerError> {
    let Some(headers) = config.get("headers") else {
        return Ok(Vec::new());
    };

    let Value::Object(map) = headers else {
        return Err(RunnerError::InvalidFetchConfig(
            "fetch node config.headers must be an object".to_string(),
        ));
    };

    map.iter()
        .map(|(key, value)| {
            if key.trim().is_empty() {
                return Err(RunnerError::InvalidFetchConfig(
                    "fetch node headers cannot contain empty keys".to_string(),
                ));
            }

            match value {
                Value::Null => Ok((key.clone(), String::new())),
                Value::String(text) => Ok((key.clone(), text.clone())),
                Value::Bool(boolean) => Ok((key.clone(), boolean.to_string())),
                Value::Number(number) => Ok((key.clone(), number.to_string())),
                Value::Array(_) | Value::Object(_) => Ok((
                    key.clone(),
                    serde_json::to_string(value).map_err(|error| {
                        RunnerError::InvalidFetchConfig(format!(
                            "failed to serialize header {key}: {error}"
                        ))
                    })?,
                )),
            }
        })
        .collect()
}

fn build_fetch_client(timeout_ms: Option<u64>) -> Result<Client, RunnerError> {
    let mut builder = Client::builder();
    if let Some(timeout_ms) = timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }

    builder
        .build()
        .map_err(|error| RunnerError::FetchRequest(error.to_string()))
}

fn build_fetch_request(
    client: &Client,
    method: &Method,
    url: &str,
    headers: Vec<(String, String)>,
    request: &Value,
) -> Result<RequestBuilder, RunnerError> {
    let mut builder = client.request(method.clone(), url);
    for (key, value) in headers {
        builder = builder.header(&key, value);
    }

    if *method == Method::GET {
        let query = value_to_query_pairs(request)?;
        if !query.is_empty() {
            builder = builder.query(&query);
        }
        return Ok(builder);
    }

    if *method == Method::POST {
        return Ok(builder.json(request));
    }

    Err(RunnerError::InvalidFetchConfig(format!(
        "fetch node only supports GET and POST, got {}",
        method.as_str()
    )))
}

fn value_to_query_pairs(value: &Value) -> Result<Vec<(String, String)>, RunnerError> {
    let Value::Object(map) = value else {
        if value.is_null() {
            return Ok(Vec::new());
        }

        return Err(RunnerError::InvalidFetchConfig(
            "GET fetch inputMapping must resolve to an object".to_string(),
        ));
    };

    let mut pairs = Vec::new();
    for (key, value) in map {
        if value.is_null() {
            continue;
        }

        let rendered = match value {
            Value::String(text) => text.clone(),
            Value::Bool(boolean) => boolean.to_string(),
            Value::Number(number) => number.to_string(),
            Value::Array(_) | Value::Object(_) => {
                serde_json::to_string(value).map_err(|error| {
                    RunnerError::InvalidFetchConfig(format!(
                        "failed to serialize query param {key}: {error}"
                    ))
                })?
            }
            Value::Null => continue,
        };
        pairs.push((key.clone(), rendered));
    }

    Ok(pairs)
}

fn build_fetch_response_payload(response: Response) -> Result<Value, RunnerError> {
    let status = response.status();
    let url = response.url().to_string();
    let headers = response
        .headers()
        .iter()
        .map(|(key, value)| {
            (
                key.to_string(),
                Value::String(value.to_str().unwrap_or_default().to_string()),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body_text = response
        .text()
        .map_err(|error| RunnerError::FetchRequest(error.to_string()))?;
    let data = parse_fetch_response_body(&content_type, &body_text);

    Ok(json!({
        "url": url,
        "response": {
            "ok": status.is_success(),
            "status": status.as_u16(),
            "statusText": status.canonical_reason().unwrap_or_default(),
            "headers": headers,
            "contentType": content_type
        },
        "data": data
    }))
}

fn parse_fetch_response_body(content_type: &str, body_text: &str) -> Value {
    if body_text.trim().is_empty() {
        return Value::Null;
    }

    if content_type.contains("json") {
        return serde_json::from_str(body_text)
            .unwrap_or_else(|_| Value::String(body_text.to_string()));
    }

    Value::String(body_text.to_string())
}
// endregion Fetch 节点辅助函数

// region Shell 节点辅助函数
fn resolve_shell_execution_spec(
    config: &Value,
    command: &str,
) -> Result<ShellExecutionSpec, RunnerError> {
    let shell = config
        .get("shell")
        .or_else(|| config.get("program"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("sh")
        .to_string();
    let working_directory = config
        .get("workingDirectory")
        .or_else(|| config.get("cwd"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(resolve_shell_working_directory)
        .transpose()?;
    let env = match config.get("env") {
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                if key.trim().is_empty() {
                    return Err(RunnerError::InvalidShellConfig(
                        "shell env cannot contain empty keys".to_string(),
                    ));
                }

                Ok((key.clone(), value_to_env_string(value)?))
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(RunnerError::InvalidShellConfig(
                "shell node config.env must be an object".to_string(),
            ));
        }
        None => Vec::new(),
    };

    Ok(ShellExecutionSpec {
        shell,
        command: command.to_string(),
        working_directory,
        env,
    })
}

fn value_to_env_string(value: &Value) -> Result<String, RunnerError> {
    match value {
        Value::Null => Ok(String::new()),
        Value::String(text) => Ok(text.clone()),
        Value::Bool(boolean) => Ok(boolean.to_string()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)
            .map_err(|error| RunnerError::InvalidShellConfig(error.to_string())),
    }
}

fn resolve_shell_working_directory(path: &str) -> Result<PathBuf, RunnerError> {
    let file_path = Path::new(path);
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| RunnerError::InvalidShellConfig(error.to_string()))?
            .join(file_path)
    };

    absolute.canonicalize().map_err(|error| {
        RunnerError::InvalidShellConfig(format!(
            "failed to resolve shell working directory {}: {error}",
            absolute.display()
        ))
    })
}

fn execute_shell_command(
    spec: &ShellExecutionSpec,
    context: &NodeExecutionContext<'_>,
    params: &Value,
    timeout_ms: Option<u64>,
) -> Result<ShellProcessOutput, RunnerError> {
    let params_json = serde_json::to_string(params)
        .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let trigger_json = serde_json::to_string(context.trigger)
        .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let input_json = serde_json::to_string(context.input)
        .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let state_json = serde_json::to_string(context.state)
        .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let env_json = serde_json::to_string(&env_to_value(context.env))
        .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;

    let mut command = Command::new(&spec.shell);
    command
        .args(["-lc", &spec.command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("WORKFLOW_RUN_ID", context.run_id)
        .env("WORKFLOW_KEY", context.workflow_key)
        .env("WORKFLOW_VERSION", context.workflow_version.to_string())
        .env("WORKFLOW_TRIGGER", trigger_json)
        .env("WORKFLOW_INPUT", input_json)
        .env("WORKFLOW_STATE", state_json)
        .env("WORKFLOW_PARAMS", &params_json)
        .env("WORKFLOW_ENV", env_json);
    if let Some(working_directory) = &spec.working_directory {
        command.current_dir(working_directory);
    }
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RunnerError::ShellExecution("failed to open shell stdin".to_string()))?;
        stdin
            .write_all(params_json.as_bytes())
            .map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    }

    let output = wait_for_process_output(child, timeout_ms, "shell")?;
    let exit_code = output.status.code().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(RunnerError::ShellExecution(if stderr.is_empty() {
            format!("shell command exited with status {}", output.status)
        } else {
            stderr
        }));
    }

    Ok(ShellProcessOutput {
        result: json!({
            "shell": spec.shell,
            "command": spec.command,
            "workingDirectory": spec.working_directory.as_ref().map(|path| path.display().to_string()),
            "exitCode": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "data": parse_shell_stdout(&stdout)
        }),
        logs: shell_logs_from_stderr(&stderr),
    })
}

fn parse_shell_stdout(stdout: &str) -> Value {
    if stdout.is_empty() {
        return Value::Null;
    }

    serde_json::from_str(stdout).unwrap_or_else(|_| Value::String(stdout.to_string()))
}

fn shell_logs_from_stderr(stderr: &str) -> Vec<NodeLogRecord> {
    stderr
        .lines()
        .filter_map(|line| {
            let message = line.trim();
            if message.is_empty() {
                return None;
            }

            Some(NodeLogRecord {
                level: "warn".to_string(),
                message: message.to_string(),
            })
        })
        .collect()
}
// endregion Shell 节点辅助函数

// region Code 节点辅助函数
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

    let base_dir = resolve_code_base_dir(node)?;

    if let Some(source_path) = node
        .config
        .get("sourcePath")
        .or_else(|| node.config.get("filePath"))
        .and_then(Value::as_str)
    {
        let resolved = resolve_code_file_path(base_dir.as_deref(), source_path)?;
        let source = fs::read_to_string(&resolved).map_err(|error| {
            RunnerError::CodeExecution(format!(
                "failed to read code source file {}: {error}",
                resolved.display()
            ))
        })?;
        return Ok(CodeExecutionSpec::InlineSource(source));
    }

    if let Some(module_path) = node.config.get("modulePath").and_then(Value::as_str) {
        let resolved = resolve_code_file_path(base_dir.as_deref(), module_path)?;
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

fn resolve_code_base_dir(node: &NodeDefinition) -> Result<Option<PathBuf>, RunnerError> {
    let Some(base_dir) = node
        .config
        .get("baseDir")
        .or_else(|| node.config.get("workingDirectory"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };

    resolve_code_file_path(None, base_dir).map(Some)
}

fn resolve_code_file_path(base_dir: Option<&Path>, path: &str) -> Result<PathBuf, RunnerError> {
    let file_path = Path::new(path);
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        match base_dir {
            Some(base_dir) => base_dir.join(file_path),
            None => std::env::current_dir()
                .map_err(|error| RunnerError::CodeExecution(error.to_string()))?
                .join(file_path),
        }
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

    let output = wait_for_process_output(child, timeout_ms, "code")?;

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

fn wait_for_process_output(
    mut child: std::process::Child,
    timeout_ms: Option<u64>,
    process_name: &str,
) -> Result<std::process::Output, RunnerError> {
    let Some(timeout_ms) = timeout_ms else {
        return child
            .wait_with_output()
            .map_err(|error| match process_name {
                "shell" => RunnerError::ShellExecution(error.to_string()),
                _ => RunnerError::CodeExecution(error.to_string()),
            });
    };

    let started_at = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait().map_err(|error| match process_name {
            "shell" => RunnerError::ShellExecution(error.to_string()),
            _ => RunnerError::CodeExecution(error.to_string()),
        })? {
            Some(_) => {
                return child
                    .wait_with_output()
                    .map_err(|error| match process_name {
                        "shell" => RunnerError::ShellExecution(error.to_string()),
                        _ => RunnerError::CodeExecution(error.to_string()),
                    });
            }
            None if started_at.elapsed() >= timeout => {
                child.kill().map_err(|error| match process_name {
                    "shell" => RunnerError::ShellExecution(error.to_string()),
                    _ => RunnerError::CodeExecution(error.to_string()),
                })?;
                let _ = child.wait();
                let message = format!("{process_name} node exceeded timeout of {timeout_ms}ms");
                return Err(match process_name {
                    "shell" => RunnerError::ShellExecution(message),
                    _ => RunnerError::CodeExecution(message),
                });
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}
// endregion Code 节点辅助函数

// region Code 节点 JavaScript 引导脚本
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
// endregion Code 节点 JavaScript 引导脚本

// region Shell / Code 节点数据结构
struct ShellProcessOutput {
    result: Value,
    logs: Vec<NodeLogRecord>,
}

struct ShellExecutionSpec {
    shell: String,
    command: String,
    working_directory: Option<PathBuf>,
    env: Vec<(String, String)>,
}

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
// endregion Shell / Code 节点数据结构

// region 上下文与子流程辅助函数
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
        return deserialize_workflow_definition(definition)
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
// endregion 上下文与子流程辅助函数
