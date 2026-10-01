use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool '{0}' not found in registry")]
    NotFound(String),

    #[error("Invalid parameters for tool '{name}': {reason}")]
    InvalidParameters {
        name: String,
        reason: String,
    },

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Tool execution timed out after {timeout_secs}s")]
    Timeout {
        timeout_secs: u64,
    },

    #[error("Tool execution permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error during tool operation: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ToolError>;

