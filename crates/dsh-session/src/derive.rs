use crate::event::{SessionEvent, SessionEventRecord};
use dsh_types::Message;

/// Pure function to project an append-only event log into a sequence of
/// model-visible conversation messages for LLM context assembly.
pub fn derive_messages(records: &[SessionEventRecord]) -> Vec<Message> {
    let mut messages = Vec::new();

    for record in records {
        match &record.event {
            SessionEvent::SystemMessage(sys) => {
                messages.push(Message::System(sys.clone()));
            }
            SessionEvent::UserMessage(user) => {
                messages.push(Message::User(user.clone()));
            }
            SessionEvent::AssistantMessage(assistant) => {
                messages.push(Message::Assistant(assistant.clone()));
            }
            SessionEvent::ToolResult { result, .. } => {
                messages.push(Message::Tool(result.clone()));
            }
            // TurnStart, TurnEnd, StepStart, StepEnd, ToolCall are lifecycle/audit records
            // that do not project directly into the prompt surface.
            SessionEvent::TurnStart { .. }
            | SessionEvent::TurnEnd { .. }
            | SessionEvent::StepStart { .. }
            | SessionEvent::StepEnd { .. }
            | SessionEvent::ToolCall { .. } => {}
        }
    }

    messages
}

