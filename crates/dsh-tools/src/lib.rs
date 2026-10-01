pub mod builtin;
pub mod error;
pub mod pipeline;
pub mod registry;
pub mod spill;
pub mod tool;

pub use builtin::{BashTool, FileReadTool, FileWriteTool};
pub use error::{Result, ToolError};
pub use pipeline::ExecutionPipeline;
pub use registry::ToolRegistry;
pub use spill::{process_output_spill, SpillConfig};
pub use tool::{Tool, ToolSchema};

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_types::ToolCallId;

    #[tokio::test]
    async fn test_tool_registry_and_schema_export() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool::new());
        registry.register(FileReadTool::new());
        registry.register(FileWriteTool);

        assert!(registry.contains("bash"));
        assert!(registry.contains("read_file"));
        assert!(registry.contains("write_file"));

        let schemas = registry.export_openai_tools();
        assert_eq!(schemas.len(), 3);
        assert_eq!(schemas[0]["type"], "function");
    }

    #[tokio::test]
    async fn test_bash_tool_execution() {
        let tool = BashTool::new();
        let call_id = ToolCallId::generate();
        let res = tool
            .execute(&call_id, serde_json::json!({ "cmd": "echo 'deepseek-harness-rust'" }))
            .await
            .unwrap();

        assert_eq!(res.tool_call_id, call_id);
        assert!(!res.is_error);
        assert!(res.content.contains("deepseek-harness-rust"));
    }

    #[tokio::test]
    async fn test_bash_tool_error_exit() {
        let tool = BashTool::new();
        let call_id = ToolCallId::generate();
        let res = tool
            .execute(&call_id, serde_json::json!({ "cmd": "exit 42" }))
            .await
            .unwrap();

        assert_eq!(res.tool_call_id, call_id);
        assert!(res.is_error);
        assert!(res.content.contains("status code 42"));
    }

    #[tokio::test]
    async fn test_fs_tools_write_and_read() {
        let temp_dir = std::env::temp_dir().join("dsh-test-fs");
        let test_file = temp_dir.join("sample.txt");
        let test_file_str = test_file.to_str().unwrap().to_string();

        let write_tool = FileWriteTool;
        let call_id1 = ToolCallId::generate();
        let write_res = write_tool
            .execute(
                &call_id1,
                serde_json::json!({
                    "path": test_file_str,
                    "content": "line1\nline2\nline3"
                }),
            )
            .await
            .unwrap();
        assert!(!write_res.is_error);

        let read_tool = FileReadTool::new();
        let call_id2 = ToolCallId::generate();
        let read_res = read_tool
            .execute(
                &call_id2,
                serde_json::json!({
                    "path": test_file_str,
                    "offset_lines": 1,
                    "limit_lines": 1
                }),
            )
            .await
            .unwrap();
        assert!(!read_res.is_error);
        assert_eq!(read_res.content, "line2");
    }

    #[tokio::test]
    async fn test_pipeline_unknown_tool() {
        let registry = ToolRegistry::new();
        let pipeline = ExecutionPipeline::new(registry);

        let call_id = ToolCallId::generate();
        let res = pipeline
            .dispatch(&call_id, "non_existent", serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(res.tool_call_id, call_id);
        assert!(res.is_error);
        assert!(res.content.contains("Unknown tool"));
    }
}

