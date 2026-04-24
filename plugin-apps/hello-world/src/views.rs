use axum::Json;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

pub(crate) fn json_response<T>(status: StatusCode, payload: &T, trace_id: Option<&str>) -> Response
where
    T: Serialize,
{
    let mut response = (status, Json(payload)).into_response();
    if let Some(trace_id) = trace_id {
        if let Ok(value) = HeaderValue::from_str(trace_id) {
            response.headers_mut().insert("X-Trace-Id", value);
        }
    }
    response
}
