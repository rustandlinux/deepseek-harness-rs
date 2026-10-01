use dsh_llm::LlmError;
use dsh_tools::ToolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM failure during step: {0}")]
    Llm(#[from] LlmError),

    #[error("Tool execution error: {0}")]
    Tool(#[from] ToolError),

    #[error("Turn was aborted: {0}")]
    Aborted(String),

    #[error("Agent loop exceeded maximum step limit ({0})")]
    MaxStepsExceeded(usize),

    #[error("Internal agent error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;

