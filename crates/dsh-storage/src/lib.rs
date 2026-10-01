pub mod error;
pub mod jsonl;
pub mod repair;
pub mod sqlite;
pub mod store;

pub use error::{Result, StorageError};
pub use jsonl::JsonlSessionStore;
pub use repair::repair_session_records;
pub use sqlite::SqliteSessionStore;
pub use store::{SessionMetadata, SessionStore};

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_session::{SessionEvent, SessionEventRecord, TurnEndReason};
    use dsh_types::{ContentBlock, SessionId, UserMessage};

    #[tokio::test]
    async fn test_jsonl_store_append_and_load() {
        let temp_dir = std::env::temp_dir().join(format!("dsh_test_jsonl_{}", uuid::Uuid::new_v4().simple()));
        let store = JsonlSessionStore::new(&temp_dir);

        let sid = SessionId::generate();
        store.create_session(&sid).await.unwrap();

        let records = vec![
            SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
            SessionEventRecord::new(1.into(), SessionEvent::UserMessage(UserMessage::text("hello jsonl"))),
            SessionEventRecord::new(2.into(), SessionEvent::TurnEnd { turn: 1, reason: TurnEndReason::Complete }),
        ];

        store.append_events(&sid, &records).await.unwrap();

        let loaded = store.load_events(&sid).await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[1].seq.as_u64(), 1);

        let list = store.list_sessions().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, sid);
        assert_eq!(list[0].event_count, 3);
    }

    #[tokio::test]
    async fn test_sqlite_store_append_and_load() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let sid = SessionId::generate();
        store.create_session(&sid).await.unwrap();

        let records = vec![
            SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
            SessionEventRecord::new(1.into(), SessionEvent::UserMessage(UserMessage::text("hello sqlite"))),
            SessionEventRecord::new(2.into(), SessionEvent::TurnEnd { turn: 1, reason: TurnEndReason::Complete }),
        ];

        store.append_events(&sid, &records).await.unwrap();

        let loaded = store.load_events(&sid).await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[1].seq.as_u64(), 1);

        let list = store.list_sessions().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, sid);
        assert_eq!(list[0].event_count, 3);
    }

    #[tokio::test]
    async fn test_sqlite_crash_recovery_on_load() {
        let store = SqliteSessionStore::open_in_memory().unwrap();
        let sid = SessionId::generate();

        // 模拟异常崩溃：仅记录了 turn/start、step/start 与悬空的 tool/call，没有后续 step/end 和 turn/end
        let incomplete_records = vec![
            SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
            SessionEventRecord::new(1.into(), SessionEvent::StepStart { turn: 1, step: 1 }),
            SessionEventRecord::new(
                2.into(),
                SessionEvent::ToolCall {
                    turn: 1,
                    step: 1,
                    call: ContentBlock::tool_call("call_interrupted", "bash", serde_json::json!({ "cmd": "sleep 100" })),
                },
            ),
        ];

        store.append_events(&sid, &incomplete_records).await.unwrap();

        // load_events 会自动触发 repair_session_records 逻辑
        let repaired = store.load_events(&sid).await.unwrap();

        // 期望得到：原3条 + 1条合成ToolResult + 1条合成StepEnd + 1条合成TurnEnd = 6条
        assert_eq!(repaired.len(), 6);
        assert_eq!(repaired[3].seq.as_u64(), 3);
        assert!(matches!(&repaired[3].event, SessionEvent::ToolResult { .. }));
        assert_eq!(repaired[4].seq.as_u64(), 4);
        assert!(matches!(&repaired[4].event, SessionEvent::StepEnd { .. }));
        assert_eq!(repaired[5].seq.as_u64(), 5);
        assert!(matches!(&repaired[5].event, SessionEvent::TurnEnd { .. }));
    }
}

