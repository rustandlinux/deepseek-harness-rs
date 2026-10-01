use serde::{Deserialize, Serialize};

/// Token usage metadata returned by the provider.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_hit_tokens: Option<u32>,
}

/// A streaming token chunk emitted during LLM generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamChunk {
    /// Incremental visible response text.
    TextDelta(String),
    /// Incremental reasoning/thinking token (DeepSeek R1 reasoning content).
    ReasoningDelta(String),
    /// Incremental fragment of a tool call.
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: String,
    },
    /// Usage metrics report.
    Usage(Usage),
    /// End-of-stream sentinel.
    Done,
}
