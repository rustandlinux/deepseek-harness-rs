use dsh_acp::AcpServer;
use dsh_llm::OpenAiAdapter;
use dsh_tools::{BashTool, FileReadTool, FileWriteTool, ToolRegistry};
use std::sync::Arc;
use tokio::io::{stdin, stdout, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 监听标准输入输出，对外提供 ACP (Agent Client Protocol) JSON-RPC 服务
    let adapter = Arc::new(OpenAiAdapter::from_env());

    let mut registry = ToolRegistry::new();
    registry.register(BashTool::new());
    registry.register(FileReadTool::new());
    registry.register(FileWriteTool);

    let mut server = AcpServer::new(adapter, registry)
        .with_system_prompt("You are an expert coding assistant connected via Agent Client Protocol (ACP).");

    let reader = BufReader::new(stdin());
    let writer = stdout();

    server.run_stdio(reader, writer).await?;
    Ok(())
}

