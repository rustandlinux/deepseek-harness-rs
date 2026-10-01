use crate::id::ToolCallId;
use serde::{Deserialize, Serialize};

/// Strongly typed content block within a conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ContentBlock {
    /// Plain text content.
    Text { text: String },
    /// Reasoning/thinking chain (e.g. DeepSeek-R1 reasoning content).
    Reasoning { reasoning: String },
    /// Visual image asset reference or data URL.
    Image {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
    },
    /// File attachment reference.
    File {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },
    /// Tool execution request emitted by the model.
    ToolCall {
        id: ToolCallId,
        name: String,
        arguments: serde_json::Value,
    },
}

impl ContentBlock {
    /// Helper to construct a text block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Helper to construct a reasoning block.
    pub fn reasoning(reasoning: impl Into<String>) -> Self {
        Self::Reasoning {
            reasoning: reasoning.into(),
        }
    }

    /// Helper to construct a tool-call block.
    pub fn tool_call(
        id: impl Into<ToolCallId>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    /// Borrow text if this is a Text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Borrow reasoning if this is a Reasoning block.
    pub fn as_reasoning(&self) -> Option<&str> {
        match self {
            Self::Reasoning { reasoning } => Some(reasoning),
            _ => None,
        }
    }
}
