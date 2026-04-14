use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::{NodeExecutor, resolve_mapping, wait_for_process_output};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult, NodeLogRecord};
use crate::core::template::env_to_value;
use crate::error::RunnerError;

pub(super) struct CodeExecutor;

impl NodeExecutor for CodeExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Code
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let language = parse_code_language(node)?;
        let params = resolve_mapping(node, context);
        let spec = resolve_code_execution_spec(node)?;
        let process_output = execute_code(&spec, language, context, &params, node.timeout_ms)?;

        Ok(build_code_result(process_output.result).with_logs(process_output.logs))
    }
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

fn execute_code(
    spec: &CodeExecutionSpec,
    language: CodeLanguage,
    context: &NodeExecutionContext<'_>,
    params: &Value,
    timeout_ms: Option<u64>,
) -> Result<CodeProcessOutput, RunnerError> {
    let mut payload = json!({
        "trigger": context.trigger,
        "input": context.input,
        "state": context.state,
        "env": env_to_value(context.env),
        "params": params,
        "requiresTypeTransform": language == CodeLanguage::TypeScript
    });
    let mut temporary_module = None;
    match (language, spec) {
        (CodeLanguage::JavaScript, CodeExecutionSpec::InlineSource(source)) => {
            payload["source"] = Value::String(source.clone());
        }
        (CodeLanguage::TypeScript, CodeExecutionSpec::InlineSource(source)) => {
            let module = TemporaryCodeModule::create(source)?;
            payload["modulePath"] = Value::String(module.path().to_string_lossy().to_string());
            temporary_module = Some(module);
        }
        (_, CodeExecutionSpec::Module {
            module_path,
            export_name,
        }) => {
            payload["modulePath"] = Value::String(module_path.clone());
            payload["exportName"] = Value::String(export_name.clone());
        }
    }
    let request = serde_json::to_vec(&payload)
        .map_err(|error| RunnerError::CodeExecution(error.to_string()))?;

    let mut command = Command::new("node");
    if language == CodeLanguage::TypeScript {
        command.arg("--experimental-transform-types");
    }
    let mut child = command
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
    drop(temporary_module);

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

fn parse_code_language(node: &NodeDefinition) -> Result<CodeLanguage, RunnerError> {
    let language = node
        .config
        .get("language")
        .or_else(|| node.config.get("lang"))
        .and_then(Value::as_str)
        .unwrap_or("js");

    match language {
        "js" | "javascript" => Ok(CodeLanguage::JavaScript),
        "ts" | "typescript" => Ok(CodeLanguage::TypeScript),
        _ => Err(RunnerError::CodeExecution(format!(
            "node {} only supports js/javascript/ts/typescript, got {}",
            node.id, language
        ))),
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
  params = null,
  requiresTypeTransform = false
} = payload;
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const [majorRaw = '0', minorRaw = '0'] = (process.versions.node || '0.0').split('.');
const majorVersion = Number(majorRaw || '0');
const minorVersion = Number(minorRaw || '0');
if (
  !Number.isFinite(majorVersion) ||
  !Number.isFinite(minorVersion) ||
  majorVersion < 22 ||
  (requiresTypeTransform && majorVersion === 22 && minorVersion < 20)
) {
  throw new Error(
    requiresTypeTransform
      ? `TypeScript code node requires Node.js 22.20+, got ${process.version}`
      : `Code node requires Node.js 22+, got ${process.version}`
  );
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodeLanguage {
    JavaScript,
    TypeScript,
}

enum CodeExecutionSpec {
    InlineSource(String),
    Module {
        module_path: String,
        export_name: String,
    },
}

struct TemporaryCodeModule {
    path: PathBuf,
}

impl TemporaryCodeModule {
    fn create(source: &str) -> Result<Self, RunnerError> {
        static TEMP_CODE_MODULE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

        let unique = TEMP_CODE_MODULE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ses-runner-code-{}-{}-{}.mts",
            std::process::id(),
            unique,
            std::thread::current().name().unwrap_or("worker")
        ));
        let wrapped = format!(
            "export default async function (trigger, input, state, env, params) {{\n{source}\n}}\n"
        );
        fs::write(&path, wrapped).map_err(|error| {
            RunnerError::CodeExecution(format!(
                "failed to write temporary TypeScript module {}: {error}",
                path.display()
            ))
        })?;

        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryCodeModule {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
