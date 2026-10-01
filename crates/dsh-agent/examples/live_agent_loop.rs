use dsh_agent::ReactLoopAgent;
use dsh_llm::OpenAiAdapter;
use dsh_tools::{BashTool, FileReadTool, FileWriteTool, ToolRegistry};
use dsh_types::SessionId;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Live ReAct Agent Loop ===");

    // 1. Configure OpenAiAdapter using environment variables
    let adapter = Arc::new(OpenAiAdapter::from_env());

    // 2. Register tools
    let mut registry = ToolRegistry::new();
    registry.register(BashTool::new());
    registry.register(FileReadTool::new());
    registry.register(FileWriteTool);

    let session_id = SessionId::generate();
    println!("Session ID: {}", session_id);

    // 3. Initialize ReactLoopAgent
    let mut agent = ReactLoopAgent::new(session_id, adapter, registry)
        .with_system_prompt("You are an autonomous engineering agent running on DeepSeek Harness Rust. Use tools when needed to complete the user's task.");

    // 4. Run turn with real task requiring tool execution
    let task = "Please use the bash tool to run 'date -u' to check the current UTC date, and then report the result.";
    println!("Task: {}", task);
    println!("Running turn...");

    let cancel = CancellationToken::new();
    let reason = agent.run_turn(task, cancel).await?;
    println!("Turn completed with reason: {:?}\n", reason);

    // 5. Inspect the recorded session log
    let session = agent.session();
    let sess = session.lock().await;

    println!("=== Recorded Session Event Log ({} events) ===", sess.len());
    for record in sess.records() {
        println!("[seq: {}] {:?}", record.seq, record.event);
    }

    println!("\n=== Full ReAct Agent Loop Verified Successfully! ===");
    Ok(())
}
