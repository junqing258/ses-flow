use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

pub fn health() -> HealthResponse {
    HealthResponse { status: "ok" }
}
