use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("HTTP network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error response (HTTP {status}): {body}")]
    Api {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("JSON parse/serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SSE stream protocol error: {0}")]
    Protocol(String),

    #[error("Stream assembler error: {0}")]
    Assembler(String),
}

pub type Result<T> = std::result::Result<T, LlmError>;

