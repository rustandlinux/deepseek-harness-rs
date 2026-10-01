pub mod error;
pub mod inbox;
pub mod react;

pub use error::{AgentError, Result};
pub use inbox::{Inbox, InboxItem};
pub use react::ReactLoopAgent;

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_llm::{BoxStream, LlmAdapter, StreamChunk};
    use dsh_session::TurnEndReason;
    use dsh_tools::{BashTool, ToolRegistry};
    use dsh_types::{Message, SessionId};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    struct MockLlm {
        call_count: AtomicUsize,
    }

    impl LlmAdapter for MockLlm {
        fn stream<'a>(
            &'a self,
            _messages: &'a [Message],
            _tools: Option<&'a [serde_json::Value]>,
        ) -> BoxStream<'a, dsh_llm::Result<StreamChunk>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            let chunks = if count == 0 {
                // First step: model emits a tool call
                vec![
                    Ok(StreamChunk::ToolCallDelta {
                        index: 0,
                        id: Some("call_test_01".to_string()),
                        name: Some("bash".to_string()),
                        arguments_delta: "{\"cmd\": \"echo mock-output\"}".to_string(),
                    }),
                    Ok(StreamChunk::Done),
                ]
            } else {
                // Second step: model completes with text
                vec![
                    Ok(StreamChunk::TextDelta("Task complete!".to_string())),
                    Ok(StreamChunk::Done),
                ]
            };

            Box::pin(futures_util::stream::iter(chunks))
        }
    }

    #[tokio::test]
    async fn test_agent_react_multi_step_loop() {
        let mock_llm = Arc::new(MockLlm {
            call_count: AtomicUsize::new(0),
        });

        let mut registry = ToolRegistry::new();
        registry.register(BashTool::new());

        let mut agent = ReactLoopAgent::new(SessionId::generate(), mock_llm, registry)
            .with_system_prompt("You are a mock agent");

        let cancel = CancellationToken::new();
        let reason = agent.run_turn("Run the bash check", cancel).await.unwrap();

        assert_eq!(reason, TurnEndReason::Complete);

        // Verify session event log
        let session = agent.session();
        let sess = session.lock().await;

        let event_types: Vec<&str> = sess
            .records()
            .iter()
            .map(|r| match &r.event {
                dsh_session::SessionEvent::TurnStart { .. } => "turn/start",
                dsh_session::SessionEvent::SystemMessage(_) => "system/message",
                dsh_session::SessionEvent::UserMessage(_) => "user/message",
                dsh_session::SessionEvent::StepStart { .. } => "step/start",
                dsh_session::SessionEvent::AssistantMessage(_) => "assistant/message",
                dsh_session::SessionEvent::ToolCall { .. } => "tool/call",
                dsh_session::SessionEvent::ToolResult { .. } => "tool/result",
                dsh_session::SessionEvent::StepEnd { .. } => "step/end",
                dsh_session::SessionEvent::TurnEnd { .. } => "turn/end",
            })
            .collect();

        assert_eq!(
            event_types,
            vec![
                "turn/start",
                "system/message",
                "user/message",
                "step/start",
                "assistant/message",
                "tool/call",
                "tool/result",
                "step/end",
                "step/start",
                "assistant/message",
                "step/end",
                "turn/end"
            ]
        );
    }

    #[tokio::test]
    async fn test_agent_cancellation() {
        let mock_llm = Arc::new(MockLlm {
            call_count: AtomicUsize::new(0),
        });
        let registry = ToolRegistry::new();

        let mut agent = ReactLoopAgent::new(SessionId::generate(), mock_llm, registry);

        let cancel = CancellationToken::new();
        cancel.cancel(); // Pre-cancelled

        let reason = agent.run_turn("Will cancel", cancel).await.unwrap();
        assert_eq!(reason, TurnEndReason::Aborted);
    }
}

