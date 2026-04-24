use chrono::Utc;
use std::sync::Arc;

use super::{NodeExecutor, resolve_config, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;
use crate::services::WorkflowServices;
use crate::services::{
    HttpPluginExecutionContext, HttpPluginExecutionRequest, NodeTransport, PluginResponseEnvelope, build_http_client,
    build_plugin_headers, extract_request_id_from_value, extract_trace_id_from_value, inject_plugin_log_metadata,
};

pub(super) struct PluginExecutor {
    pub services: Arc<WorkflowServices>,
}

impl NodeExecutor for PluginExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::Plugin("plugin:*".to_string())
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let request_input = resolve_mapping(node, context);
        let resolved_config = resolve_config(node, context, &request_input);
        let descriptor = self
            .services
            .node_descriptors
            .resolve_by_runner_type(node.node_type.as_str())
            .ok_or_else(|| {
                RunnerError::PluginExecution(format!(
                    "plugin descriptor not found for runner type {}",
                    node.node_type.as_str()
                ))
            })?;

        if !matches!(descriptor.transport, Some(NodeTransport::Http)) {
            return Err(RunnerError::PluginExecution(format!(
                "plugin {} is not using transport=http",
                descriptor.id
            )));
        }

        let endpoint = descriptor
            .endpoint
            .clone()
            .ok_or_else(|| RunnerError::PluginExecution(format!("plugin {} is missing endpoint", descriptor.id)))?;
        let request_id = extract_request_id_from_value(context.trigger)
            .or_else(|| extract_request_id_from_value(&request_input))
            .or_else(|| extract_request_id_from_value(context.state))
            .unwrap_or_else(|| context.run_id.to_string());
        let trace_id = extract_trace_id_from_value(context.trigger)
            .or_else(|| extract_trace_id_from_value(&request_input))
            .or_else(|| extract_trace_id_from_value(context.state));
        let timeout_ms = node.timeout_ms.or(descriptor.timeout_ms);

        let payload = HttpPluginExecutionRequest {
            plugin_id: descriptor.id.clone(),
            runner_type: node.node_type.as_str().to_string(),
            node_id: node.id.clone(),
            config: resolved_config,
            context: HttpPluginExecutionContext {
                run_id: context.run_id.to_string(),
                request_id: request_id.clone(),
                trace_id: trace_id.clone(),
                workflow_key: context.workflow_key.to_string(),
                workflow_version: context.workflow_version,
                input: request_input,
                state: context.state.clone(),
                env: context.env.clone(),
            },
        };

        let envelope = execute_http_plugin(&endpoint, &descriptor.id, timeout_ms, payload, trace_id.as_deref())?;
        let response_trace_id = envelope.trace_id.clone().or(trace_id.clone());
        let logs = inject_plugin_log_metadata(
            envelope.body.logs,
            context.run_id,
            &request_id,
            &node.id,
            response_trace_id.as_deref(),
            Utc::now(),
        );

        match envelope.body.status.as_str() {
            "success" => Ok(NodeExecutionResult::success(envelope.body.output)
                .with_state_patch(envelope.body.state_patch)
                .with_logs(logs)),
            "waiting" => {
                let signal = envelope.body.wait_signal.ok_or_else(|| {
                    RunnerError::PluginExecution(format!(
                        "plugin {} returned waiting without waitSignal payload",
                        descriptor.id
                    ))
                })?;
                Ok(NodeExecutionResult::waiting(signal, envelope.body.output)
                    .with_state_patch(envelope.body.state_patch)
                    .with_logs(logs))
            }
            "failed" => {
                let error = envelope.body.error.ok_or_else(|| {
                    RunnerError::PluginExecution(format!(
                        "plugin {} returned failed without error payload",
                        descriptor.id
                    ))
                })?;
                Ok(NodeExecutionResult::failed(error.code, error.message, error.retryable).with_logs(logs))
            }
            other => Err(RunnerError::PluginExecution(format!(
                "plugin {} returned unsupported status {}",
                descriptor.id, other
            ))),
        }
    }
}

struct HttpPluginResponse {
    body: PluginResponseEnvelope,
    trace_id: Option<String>,
}

fn execute_http_plugin(
    endpoint: &str,
    plugin_id: &str,
    timeout_ms: Option<u64>,
    payload: HttpPluginExecutionRequest,
    trace_id: Option<&str>,
) -> Result<HttpPluginResponse, RunnerError> {
    let execute_url = format!("{}/execute", endpoint.trim_end_matches('/'));
    let trace_id_owned = trace_id.map(str::to_string);

    let future = async move {
        let client = build_http_client(timeout_ms)?;
        let headers = build_plugin_headers(trace_id_owned.as_deref())?;
        let response = client
            .post(execute_url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|error| RunnerError::PluginExecution(error.to_string()))?;
        let response_trace_id = response
            .headers()
            .get("X-Trace-Id")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());
        if !response.status().is_success() {
            return Err(RunnerError::PluginExecution(format!(
                "plugin {} execute request returned {}",
                plugin_id,
                response.status()
            )));
        }
        let body = response
            .json::<PluginResponseEnvelope>()
            .await
            .map_err(|error| RunnerError::PluginExecution(error.to_string()))?;
        Ok(HttpPluginResponse {
            body,
            trace_id: response_trace_id,
        })
    };

    crate::services::block_on(future)
}
