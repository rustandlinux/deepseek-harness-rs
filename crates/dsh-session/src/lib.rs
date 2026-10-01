pub mod derive;
pub mod error;
pub mod event;
pub mod session;

pub use derive::derive_messages;
pub use error::{Result, SessionError};
pub use event::{SessionEvent, SessionEventRecord, TurnEndReason};
pub use session::Session;

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_types::{
        AssistantMessage, ContentBlock, Message, SessionId, SystemMessage, ToolResultMessage,
        UserMessage,
    };

    #[test]
    fn test_session_append_and_continuity() {
        let sid = SessionId::generate();
        let mut session = Session::new(sid.clone());
        assert_eq!(session.len(), 0);
        assert!(session.is_empty());

        let seq0 = session.append(SessionEvent::TurnStart { turn: 1 });
        assert_eq!(seq0.as_u64(), 0);

        let seq1 = session.append(SessionEvent::UserMessage(UserMessage::text("Hello")));
        assert_eq!(seq1.as_u64(), 1);

        let seq2 = session.append(SessionEvent::TurnEnd {
            turn: 1,
            reason: TurnEndReason::Complete,
        });
        assert_eq!(seq2.as_u64(), 2);
        assert_eq!(session.len(), 3);
        assert_eq!(session.last_seq().unwrap().as_u64(), 2);

        assert!(session.validate_continuity().is_ok());
    }

    #[test]
    fn test_session_continuity_failure_on_gap() {
        let sid = SessionId::generate();
        let records = vec![
            SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
            // Gap: jump from 0 to 2 directly
            SessionEventRecord::new(2.into(), SessionEvent::TurnEnd {
                turn: 1,
                reason: TurnEndReason::Complete,
            }),
        ];

        let result = Session::from_records(sid, records);
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::SequenceMismatch { expected, actual } => {
                assert_eq!(expected.as_u64(), 1);
                assert_eq!(actual.as_u64(), 2);
            }
            err => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_derive_messages() {
        let sid = SessionId::generate();
        let mut session = Session::new(sid);

        session.append(SessionEvent::TurnStart { turn: 1 });
        session.append(SessionEvent::SystemMessage(SystemMessage::new("System instructions")));
        session.append(SessionEvent::StepStart { turn: 1, step: 1 });
        session.append(SessionEvent::UserMessage(UserMessage::text("Run ls")));
        
        let tool_call = ContentBlock::tool_call("call_01", "bash", serde_json::json!({ "cmd": "ls" }));
        session.append(SessionEvent::AssistantMessage(AssistantMessage::new(vec![tool_call.clone()])));
        session.append(SessionEvent::ToolCall {
            turn: 1,
            step: 1,
            call: tool_call,
        });
        session.append(SessionEvent::ToolResult {
            turn: 1,
            step: 1,
            result: ToolResultMessage::success("call_01", "file1.txt
file2.txt"),
        });
        session.append(SessionEvent::StepEnd { turn: 1, step: 1 });
        session.append(SessionEvent::TurnEnd {
            turn: 1,
            reason: TurnEndReason::Complete,
        });

        let derived = derive_messages(session.records());
        assert_eq!(derived.len(), 4);

        match &derived[0] {
            Message::System(s) => assert_eq!(s.content, "System instructions"),
            _ => panic!("expected system"),
        }
        match &derived[1] {
            Message::User(u) => assert_eq!(u.content[0].as_text().unwrap(), "Run ls"),
            _ => panic!("expected user"),
        }
        match &derived[2] {
            Message::Assistant(a) => assert_eq!(a.content.len(), 1),
            _ => panic!("expected assistant"),
        }
        match &derived[3] {
            Message::Tool(t) => {
                assert_eq!(t.tool_call_id.as_str(), "call_01");
                assert_eq!(t.content, "file1.txt
file2.txt");
            }
            _ => panic!("expected tool"),
        }
    }
}

