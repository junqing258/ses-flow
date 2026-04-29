#[derive(Debug, Clone)]
pub struct AppConfig {
    pub runner_base_url: Option<String>,
    pub ses_auth_base_url: Option<String>,
    pub database_url: Option<String>,
    pub heartbeat_interval_secs: u64,
}

pub const DEFAULT_RUNNER_RESUME_SIGNAL: &str = "human_task_done";
pub const HEALTH_PLUGIN_ID: &str = "workstation";
pub const DEFAULT_CONNECT_STATION_ID: &str = "anonymous";

impl AppConfig {
    pub fn from_env() -> Self {
        let runner_base_url = std::env::var("RUNNER_BASE_URL").ok().map(normalize_runner_base_url);
        let ses_auth_base_url = std::env::var("SES_AUTH_BASE_URL")
            .ok()
            .map(normalize_ses_auth_base_url)
            .or_else(|| {
                runner_base_url
                    .as_ref()
                    .map(|value| auth_base_url_from_runner_base_url(value))
            });
        Self {
            runner_base_url,
            ses_auth_base_url,
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
            ses_auth_base_url: None,
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

pub(crate) fn normalize_ses_auth_base_url(base_url: String) -> String {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.ends_with("/api/auth") {
        trimmed
    } else {
        format!("{trimmed}/api/auth")
    }
}

fn auth_base_url_from_runner_base_url(runner_base_url: &str) -> String {
    let trimmed = runner_base_url.trim_end_matches('/');
    if let Some(origin) = trimmed.strip_suffix("/runner-api") {
        format!("{origin}/api/auth")
    } else {
        normalize_ses_auth_base_url(trimmed.to_string())
    }
}
