use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("workflow validation failed: {0}")]
    Validation(String),
    #[error("fetch configuration invalid: {0}")]
    InvalidFetchConfig(String),
    #[error("workflow node not found: {0}")]
    MissingNode(String),
    #[error("workflow executor not found for node type: {0}")]
    MissingExecutor(String),
    #[error("fetch request failed: {0}")]
    FetchRequest(String),
    #[error("db query configuration invalid: {0}")]
    InvalidDbConfig(String),
    #[error("db query failed: {0}")]
    DbQuery(String),
    #[error("shell configuration invalid: {0}")]
    InvalidShellConfig(String),
    #[error("shell node execution failed: {0}")]
    ShellExecution(String),
    #[error("code node execution failed: {0}")]
    CodeExecution(String),
    #[error("sub-workflow definition not found: {0}")]
    MissingSubWorkflow(String),
    #[error("sub-workflow execution failed: {0}")]
    SubWorkflow(String),
    #[error("workflow run terminated: {0}")]
    Terminated(String),
    #[error("workflow run snapshot not found: {0}")]
    MissingRunSnapshot(String),
    #[error("workflow run store error: {0}")]
    Store(String),
    #[error("resume validation failed: {0}")]
    ResumeValidation(String),
    #[error("transition resolution failed: {0}")]
    Transition(String),
    #[error("plugin registration failed: {0}")]
    PluginRegistration(String),
    #[error("plugin execution failed: {0}")]
    PluginExecution(String),
}
