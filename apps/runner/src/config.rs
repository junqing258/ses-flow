use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app::ConcurrencyConfig;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RunnerConfig {
    pub concurrency: ConcurrencyConfig,
}

impl RunnerConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|error| ConfigError::Parse {
            path: path.display().to_string(),
            error,
        })
    }

    pub fn load_optional(path: Option<impl AsRef<Path>>) -> Result<Self, ConfigError> {
        match path {
            Some(path) => Self::load(path),
            None => Ok(Self::default()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read runner config {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse runner config {path}: {error}")]
    Parse {
        path: String,
        #[source]
        error: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::RunnerConfig;

    #[test]
    fn parses_concurrency_overrides_from_toml() {
        let config: RunnerConfig = toml::from_str(
            r#"
                [concurrency]
                max_global = 12
                queue_timeout_secs = 45
                overflow_policy = "reject"

                [concurrency.per_workflow]
                default_max = 3
                "warehouse-sorting" = 9
            "#,
        )
        .expect("config should parse");

        assert_eq!(config.concurrency.max_global, 12);
        assert_eq!(config.concurrency.queue_timeout_secs, 45);
        assert_eq!(config.concurrency.per_workflow.default_max, 3);
        assert_eq!(
            config
                .concurrency
                .per_workflow
                .overrides
                .get("warehouse-sorting")
                .copied(),
            Some(9)
        );
    }
}
