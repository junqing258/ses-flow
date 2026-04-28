#[derive(Debug, Clone)]
pub struct AppConfig {
    pub runner_base_url: Option<String>,
    pub database_url: Option<String>,
    pub heartbeat_interval_secs: u64,
}

pub const DEFAULT_RUNNER_RESUME_SIGNAL: &str = "human_task_done";
pub const HEALTH_PLUGIN_ID: &str = "workstation";
pub const DEFAULT_CONNECT_WORKER_ID: &str = "anonymous";

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            runner_base_url: std::env::var("RUNNER_BASE_URL").ok().map(normalize_runner_base_url),
            database_url: std::env::var("DATABASE_URL").ok(),
            heartbeat_interval_secs: std::env::var("WORKSTATION_HEARTBEAT_INTERVAL_SECS")
                .or_else(|_| std::env::var("WCS_HEARTBEAT_INTERVAL_SECS"))
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(5),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            runner_base_url: None,
            database_url: None,
            heartbeat_interval_secs: 5,
        }
    }
}

pub(crate) fn normalize_runner_base_url(base_url: String) -> String {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.ends_with("/runner-api") {
        trimmed
    } else {
        format!("{trimmed}/runner-api")
    }
}
