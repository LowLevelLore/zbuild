use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Constraints error: {0}")]
    Constraints(String),

    #[error("Command failed: {0}")]
    CmdFailed(String),
}
