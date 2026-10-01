use dsh_agent::ReactLoopAgent;
use dsh_llm::OpenAiAdapter;
use dsh_session::TurnEndReason;
use dsh_tools::{BashTool, ToolRegistry};
use dsh_types::SessionId;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_live_agent_react_bash_interaction() {
    let adapter = Arc::new(OpenAiAdapter::from_env());
    let mut registry = ToolRegistry::new();
    registry.register(BashTool::new());

    let mut agent = ReactLoopAgent::new(SessionId::generate(), adapter, registry)
        .with_system_prompt("You are a helpful test agent. Use the bash tool when asked.");

    let cancel = CancellationToken::new();
    let res = agent
        .run_turn(
            "Please use bash to echo the exact word: REACT_SUCCESS",
            cancel,
        )
        .await;

    let reason = match res {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping live agent test if network unavailable: {e}");
            return;
        }
    };

    assert_eq!(reason, TurnEndReason::Complete);

    let session = agent.session();
    let sess = session.lock().await;

    // Verify that at least one tool call and tool result happened
    let has_tool_result = sess.records().iter().any(|r| {
        matches!(
            &r.event,
            dsh_session::SessionEvent::ToolResult { result, .. } if result.content.contains("REACT_SUCCESS")
        )
    });

    assert!(has_tool_result, "expected tool result containing REACT_SUCCESS in session log");
}

