use crate::content::ContentBlock;
use crate::id::ToolCallId;
use serde::{Deserialize, Serialize};

/// High-level participant role in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A user-origin message on the model-visible surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl UserMessage {
    pub fn new(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            source: None,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::text(text)],
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// An assistant message produced by an LLM completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl AssistantMessage {
    pub fn new(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            provider: None,
            model: None,
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::text(text)],
            provider: None,
            model: None,
        }
    }
}

/// System prompt message shaping agent behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemMessage {
    pub content: String,
}

impl SystemMessage {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

/// Result returned from executing a specific tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub tool_call_id: ToolCallId,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResultMessage {
    pub fn success(tool_call_id: impl Into<ToolCallId>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(tool_call_id: impl Into<ToolCallId>, error_msg: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: error_msg.into(),
            is_error: true,
        }
    }
}

/// Model-visible conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System(SystemMessage),
    User(UserMessage),
    Assistant(AssistantMessage),
    Tool(ToolResultMessage),
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self::System(SystemMessage::new(content))
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::User(UserMessage::text(content))
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self::Assistant(AssistantMessage::new(content))
    }

    pub fn tool_result(tool_call_id: impl Into<ToolCallId>, content: impl Into<String>, is_error: bool) -> Self {
        Self::Tool(ToolResultMessage {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error,
        })
    }
}

