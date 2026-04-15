use std::time::Duration;

use reqwest::Method;
use reqwest::{Client, RequestBuilder, Response};
use serde_json::{Value, json};
use tokio::runtime::{Builder, Handle};
use tokio::time::sleep;

use super::{NodeExecutor, resolve_config, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct FetchExecutor;

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
        let response_payload = execute_fetch_request(request_builder, context)?;

        Ok(NodeExecutionResult::success(json!({
            "method": method.as_str(),
            "url": response_payload["url"],
            "request": request,
            "response": response_payload["response"],
            "data": response_payload["data"]
        })))
    }
}

fn resolve_fetch_method(config: &Value) -> Result<Method, RunnerError> {
    let method = config
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .trim()
        .to_ascii_uppercase();

    method
        .parse::<Method>()
        .map_err(|error| RunnerError::InvalidFetchConfig(format!("unsupported fetch method {method}: {error}")))
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
                        RunnerError::InvalidFetchConfig(format!("failed to serialize header {key}: {error}"))
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

fn execute_fetch_request(
    request_builder: RequestBuilder,
    context: &NodeExecutionContext<'_>,
) -> Result<Value, RunnerError> {
    let future = async {
        let request = async {
            let response = request_builder
                .send()
                .await
                .map_err(|error| RunnerError::FetchRequest(error.to_string()))?;
            build_fetch_response_payload(response).await
        };
        tokio::pin!(request);

        loop {
            tokio::select! {
                result = &mut request => return result,
                _ = sleep(Duration::from_millis(10)) => {
                    if context.should_terminate() {
                        return Err(RunnerError::Terminated("fetch node was terminated".to_string()));
                    }
                }
            }
        }
    };

    match Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| RunnerError::FetchRequest(error.to_string()))?
            .block_on(future),
    }
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
            Value::Array(_) | Value::Object(_) => serde_json::to_string(value).map_err(|error| {
                RunnerError::InvalidFetchConfig(format!("failed to serialize query param {key}: {error}"))
            })?,
            Value::Null => continue,
        };
        pairs.push((key.clone(), rendered));
    }

    Ok(pairs)
}

async fn build_fetch_response_payload(response: Response) -> Result<Value, RunnerError> {
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
        .await
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
        return serde_json::from_str(body_text).unwrap_or_else(|_| Value::String(body_text.to_string()));
    }

    Value::String(body_text.to_string())
}
