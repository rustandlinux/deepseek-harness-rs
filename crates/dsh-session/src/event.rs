use chrono::Utc;
use dsh_types::{
    AssistantMessage, ContentBlock, SessionSeq, SystemMessage, ToolResultMessage, UserMessage,
};
use serde::{Deserialize, Serialize};

/// Reason why an agent turn completed or terminated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnEndReason {
    /// Turn completed all requested work cleanly.
    Complete,
    /// Turn reached configured step limit.
    MaxSteps,
    /// Turn reached token usage limit.
    MaxTokens,
    /// Turn was aborted / cancelled.
    Aborted,
    /// Turn encountered an unrecoverable error.
    Error(String),
}

/// An immutable fact recorded into the append-only session event log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SessionEvent {
    /// Marks the start of turn N.
    #[serde(rename = "turn/start")]
    TurnStart { turn: u32 },

    /// Marks the end of turn N with reason.
    #[serde(rename = "turn/end")]
    TurnEnd { turn: u32, reason: TurnEndReason },

    /// Marks the start of step M in turn N.
    #[serde(rename = "step/start")]
    StepStart { turn: u32, step: u32 },

    /// Marks the end of step M in turn N.
    #[serde(rename = "step/end")]
    StepEnd { turn: u32, step: u32 },

    /// System instructions or prompt section.
    #[serde(rename = "system/message")]
    SystemMessage(SystemMessage),

    /// User prompt or injected context.
    #[serde(rename = "user/message")]
    UserMessage(UserMessage),

    /// Model response containing text, reasoning or tool calls.
    #[serde(rename = "assistant/message")]
    AssistantMessage(AssistantMessage),

    /// Tool call request recorded for audit and reconstruction.
    #[serde(rename = "tool/call")]
    ToolCall {
        turn: u32,
        step: u32,
        call: ContentBlock,
    },

    /// Result returned by tool execution.
    #[serde(rename = "tool/result")]
    ToolResult {
        turn: u32,
        step: u32,
        result: ToolResultMessage,
    },
}

/// An envelope wrapping a session event with its monotonic sequence and timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEventRecord {
    pub seq: SessionSeq,
    pub timestamp_ms: i64,
    pub event: SessionEvent,
}

impl SessionEventRecord {
    pub fn new(seq: SessionSeq, event: SessionEvent) -> Self {
        Self {
            seq,
            timestamp_ms: Utc::now().timestamp_millis(),
            event,
        }
    }
}

