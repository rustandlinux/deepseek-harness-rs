use dsh_tools::{
    BashTool, ExecutionPipeline, FileReadTool, FileWriteTool, SpillConfig, ToolRegistry,
};
use dsh_types::ToolCallId;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tool Execution & Spill Pipeline Demonstration ===");

    // 1. Initialize registry with low spill threshold (1024 bytes) to demonstrate spill protection
    let mut registry = ToolRegistry::new();
    let bash_tool = BashTool::new().with_spill_config(SpillConfig {
        max_bytes: 1024,
        spill_dir: std::env::temp_dir().join("dsh-example-spills"),
    });

    registry.register(bash_tool);
    registry.register(FileReadTool::new());
    registry.register(FileWriteTool);

    println!("Registered tools: {:?}", registry.names());

    // 2. Export schemas
    let schemas = registry.export_openai_tools();
    println!("Exported {} tool schemas for OpenAI function calling.", schemas.len());

    let pipeline = ExecutionPipeline::new(registry);

    // 3. Dispatch a normal command
    println!("\n[Test 1] Executing standard bash command...");
    let call1 = ToolCallId::generate();
    let res1 = pipeline
        .dispatch(&call1, "bash", serde_json::json!({ "cmd": "uname -a" }))
        .await?;
    println!("Result 1: is_error={}, content:\n{}", res1.is_error, res1.content);

    // 4. Dispatch a large command to trigger spill protection
    println!("\n[Test 2] Executing large bash command (triggering spill)...");
    let call2 = ToolCallId::generate();
    let res2 = pipeline
        .dispatch(
            &call2,
            "bash",
            serde_json::json!({ "cmd": "for i in $(seq 1 100); do echo Line $i: data chunk output test; done" }),
        )
        .await?;
    println!("Result 2: is_error={}, preview:\n{}", res2.is_error, res2.content);

    // 5. Dispatch file write and read
    println!("\n[Test 3] Testing file write and slice read...");
    let test_path = std::env::temp_dir().join("dsh_example_file.txt");
    let test_path_str = test_path.to_str().unwrap().to_string();

    let call3 = ToolCallId::generate();
    let res3 = pipeline
        .dispatch(
            &call3,
            "write_file",
            serde_json::json!({
                "path": test_path_str,
                "content": "First Line\nSecond Line\nThird Line"
            }),
        )
        .await?;
    println!("Write result: {}", res3.content);

    let call4 = ToolCallId::generate();
    let res4 = pipeline
        .dispatch(
            &call4,
            "read_file",
            serde_json::json!({
                "path": test_path_str,
                "offset_lines": 1,
                "limit_lines": 1
            }),
        )
        .await?;
    println!("Read result (sliced): {}", res4.content);

    println!("\n=== All Tool Pipeline Tests Completed Successfully ===");
    Ok(())
}

