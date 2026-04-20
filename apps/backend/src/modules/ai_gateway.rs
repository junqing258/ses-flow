use axum::body::{Body, to_bytes};
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use crate::modules::ApiState;

const AI_GATEWAY_PROXY_BASE_PATH: &str = "/api/ai";
const HOP_BY_HOP_HEADERS: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

pub async fn proxy_root(
    state: State<ApiState>,
    method: Method,
    headers: HeaderMap,
    original_uri: OriginalUri,
    body: Body,
) -> Response {
    proxy_request_inner(state, method, headers, original_uri, body, "").await
}

pub async fn proxy_path(
    state: State<ApiState>,
    Path(path): Path<String>,
    method: Method,
    headers: HeaderMap,
    original_uri: OriginalUri,
    body: Body,
) -> Response {
    proxy_request_inner(state, method, headers, original_uri, body, &path).await
}

async fn proxy_request_inner(
    State(state): State<ApiState>,
    method: Method,
    headers: HeaderMap,
    OriginalUri(original_uri): OriginalUri,
    body: Body,
    path: &str,
) -> Response {
    let target_url = build_target_url(&state.ai_gateway_base_url, path, &original_uri);
    let body_bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read proxy request body: {error}"),
            )
                .into_response();
        }
    };

    let mut upstream_request = state.ai_gateway_client.request(method, target_url);
    for (header_name, header_value) in filter_forward_headers(&headers) {
        upstream_request = upstream_request.header(header_name, header_value);
    }
    if !body_bytes.is_empty() {
        upstream_request = upstream_request.body(body_bytes);
    }

    let upstream_response = match upstream_request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (StatusCode::BAD_GATEWAY, format!("failed to reach ai gateway: {error}")).into_response();
        }
    };

    let status = upstream_response.status();
    let response_headers = upstream_response.headers().clone();
    let response_body = Body::from_stream(upstream_response.bytes_stream());
    let mut response = Response::builder().status(status);

    for (header_name, header_value) in filter_forward_headers(&response_headers) {
        response = response.header(header_name, header_value);
    }

    match response.body(response_body) {
        Ok(response) => response,
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            format!("failed to build ai gateway proxy response: {error}"),
        )
            .into_response(),
    }
}

fn build_target_url(base_url: &str, path: &str, original_uri: &Uri) -> String {
    let trimmed_base_url = base_url.trim_end_matches('/');
    let normalized_path = if path.is_empty() {
        AI_GATEWAY_PROXY_BASE_PATH.to_string()
    } else {
        format!("{AI_GATEWAY_PROXY_BASE_PATH}/{}", path.trim_start_matches('/'))
    };
    let query = original_uri
        .query()
        .map(|value| format!("?{value}"))
        .unwrap_or_default();

    format!("{trimmed_base_url}{normalized_path}{query}")
}

fn filter_forward_headers(headers: &HeaderMap) -> Vec<(header::HeaderName, String)> {
    headers
        .iter()
        .filter(|(header_name, _)| !is_hop_by_hop_header(header_name))
        .filter(|(header_name, _)| *header_name != header::HOST && *header_name != header::CONTENT_LENGTH)
        .filter_map(|(header_name, header_value)| {
            header_value
                .to_str()
                .ok()
                .map(|value| (header_name.clone(), value.to_string()))
        })
        .collect()
}

fn is_hop_by_hop_header(header_name: &header::HeaderName) -> bool {
    HOP_BY_HOP_HEADERS
        .iter()
        .any(|value| header_name.as_str().eq_ignore_ascii_case(value))
}

pub fn resolve_ai_gateway_base_url() -> String {
    if let Ok(value) = std::env::var("AI_GATEWAY_PROXY_TARGET") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    let host = std::env::var("AI_GATEWAY_HOST")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = std::env::var("AI_GATEWAY_PORT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "6307".to_string());

    build_default_ai_gateway_base_url(&host, &port)
}

fn build_default_ai_gateway_base_url(host: &str, port: &str) -> String {
    format!("http://{host}:{port}")
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::{build_default_ai_gateway_base_url, build_target_url};

    #[test]
    fn appends_path_and_query_to_upstream_url() {
        let uri: Uri = "/api/ai/threads/session-1/messages?draft=1"
            .parse()
            .expect("uri should parse");

        let target = build_target_url("http://127.0.0.1:6307/", "threads/session-1/messages", &uri);

        assert_eq!(
            target,
            "http://127.0.0.1:6307/api/ai/threads/session-1/messages?draft=1"
        );
    }

    #[test]
    fn builds_default_upstream_target_from_host_and_port() {
        let target = build_default_ai_gateway_base_url("127.0.0.1", "6307");

        assert_eq!(target, "http://127.0.0.1:6307");
    }
}
