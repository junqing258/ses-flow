use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::{NodeExecutor, resolve_config, resolve_mapping, wait_for_process_output};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult, NodeLogRecord};
use crate::core::template::env_to_value;
use crate::error::RunnerError;

pub(super) struct ShellExecutor;

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
            .ok_or_else(|| RunnerError::InvalidShellConfig(format!("node {} is missing config.command", node.id)))?;
        let spec = resolve_shell_execution_spec(&resolved_config, command)?;
        let output = execute_shell_command(&spec, context, &payload, node.timeout_ms)?;

        Ok(NodeExecutionResult::success(output.result).with_logs(output.logs))
    }
}

fn resolve_shell_execution_spec(config: &Value, command: &str) -> Result<ShellExecutionSpec, RunnerError> {
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
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).map_err(|error| RunnerError::InvalidShellConfig(error.to_string()))
        }
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
    let params_json = serde_json::to_string(params).map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let trigger_json =
        serde_json::to_string(context.trigger).map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let input_json =
        serde_json::to_string(context.input).map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
    let state_json =
        serde_json::to_string(context.state).map_err(|error| RunnerError::ShellExecution(error.to_string()))?;
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

    let output = wait_for_process_output(child, timeout_ms, "shell", context)?;
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
                fields: Value::Null,
                run_id: None,
                request_id: None,
                node_id: None,
                trace_id: None,
                timestamp: None,
            })
        })
        .collect()
}

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
