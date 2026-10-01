pub mod content;
pub mod id;
pub mod message;
pub mod seq;

pub use content::ContentBlock;
pub use id::{AgentId, SessionId, ToolCallId};
pub use message::{AssistantMessage, Message, Role, SystemMessage, ToolResultMessage, UserMessage};
pub use seq::{SessionLogOffset, SessionSeq};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generation_and_serde() {
        let sid = SessionId::generate();
        assert!(sid.as_str().starts_with("sess_"));

        let serialized = serde_json::to_string(&sid).unwrap();
        let deserialized: SessionId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(sid, deserialized);
    }

    #[test]
    fn test_session_seq_progression() {
        let seq0 = SessionSeq::ZERO;
        assert_eq!(seq0.as_u64(), 0);

        let seq1 = seq0.next();
        assert_eq!(seq1.as_u64(), 1);
        assert!(seq1 > seq0);

        let json = serde_json::to_string(&seq1).unwrap();
        assert_eq!(json, "1");
        let parsed: SessionSeq = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, seq1);
    }

    #[test]
    fn test_content_block_serde_roundtrip() {
        let blocks = vec![
            ContentBlock::text("hello world"),
            ContentBlock::reasoning("thinking process..."),
            ContentBlock::tool_call("call_123", "bash", serde_json::json!({ "command": "ls" })),
        ];

        let json = serde_json::to_string(&blocks).unwrap();
        let decoded: Vec<ContentBlock> = serde_json::from_str(&json).unwrap();
        assert_eq!(blocks, decoded);
    }

    #[test]
    fn test_message_construction() {
        let sys = Message::system("you are an agent");
        let usr = Message::user("run cargo check");
        let tool_res = Message::tool_result("call_123", "finished successfully", false);

        match &sys {
            Message::System(s) => assert_eq!(s.content, "you are an agent"),
            _ => panic!("expected system message"),
        }

        match &usr {
            Message::User(u) => assert_eq!(u.content[0].as_text().unwrap(), "run cargo check"),
            _ => panic!("expected user message"),
        }

        match &tool_res {
            Message::Tool(t) => {
                assert_eq!(t.tool_call_id.as_str(), "call_123");
                assert!(!t.is_error);
            }
            _ => panic!("expected tool message"),
        }
    }
}

