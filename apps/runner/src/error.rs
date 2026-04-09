use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("workflow validation failed: {0}")]
    Validation(String),
    #[error("workflow node not found: {0}")]
    MissingNode(String),
    #[error("workflow executor not found for node type: {0}")]
    MissingExecutor(String),
    #[error("fetch connector not found: {0}")]
    MissingFetchConnector(String),
    #[error("action handler not found: {0}")]
    MissingActionHandler(String),
    #[error("task handler not found: {0}")]
    MissingTaskHandler(String),
    #[error("transition resolution failed: {0}")]
    Transition(String),
}
