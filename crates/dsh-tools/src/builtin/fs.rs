use crate::error::{Result, ToolError};
use crate::spill::{process_output_spill, SpillConfig};
use crate::tool::Tool;
use async_trait::async_trait;
use dsh_types::{ToolCallId, ToolResultMessage};
use serde::Deserialize;
use std::path::PathBuf;

/// Tool to read file content with line range slicing and spill protection.
pub struct FileReadTool {
    spill_config: SpillConfig,
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self {
            spill_config: SpillConfig::default(),
        }
    }
}

impl FileReadTool {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Deserialize)]
struct ReadArgs {
    path: String,
    offset_lines: Option<usize>,
    limit_lines: Option<usize>,
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read contents of a local file. Supports optional line offset and limit."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read."
                },
                "offset_lines": {
                    "type": "integer",
                    "description": "0-indexed starting line number."
                },
                "limit_lines": {
                    "type": "integer",
                    "description": "Maximum number of lines to read."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, call_id: &ToolCallId, args: serde_json::Value) -> Result<ToolResultMessage> {
        let parsed: ReadArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidParameters {
            name: self.name().to_string(),
            reason: e.to_string(),
        })?;

        let path = PathBuf::from(&parsed.path);
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(ToolError::Io)?;

        let lines: Vec<&str> = content.lines().collect();
        let offset = parsed.offset_lines.unwrap_or(0);
        let limit = parsed.limit_lines.unwrap_or(lines.len());

        let sliced: Vec<&str> = lines.into_iter().skip(offset).take(limit).collect();
        let formatted = sliced.join("\n");

        let processed = process_output_spill(&formatted, &self.spill_config)
            .map_err(ToolError::Io)?;

        Ok(ToolResultMessage::success(call_id.clone(), processed))
    }
}

/// Tool to write content to a file.
pub struct FileWriteTool;

#[derive(Debug, Deserialize)]
struct WriteArgs {
    path: String,
    content: String,
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write full content to a file at the specified path."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Target file path."
                },
                "content": {
                    "type": "string",
                    "description": "Text content to write."
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, call_id: &ToolCallId, args: serde_json::Value) -> Result<ToolResultMessage> {
        let parsed: WriteArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidParameters {
            name: self.name().to_string(),
            reason: e.to_string(),
        })?;

        let path = PathBuf::from(&parsed.path);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(ToolError::Io)?;
        }

        tokio::fs::write(&path, &parsed.content)
            .await
            .map_err(ToolError::Io)?;

        Ok(ToolResultMessage::success(
            call_id.clone(),
            format!("Successfully wrote {} bytes to {}", parsed.content.len(), parsed.path),
        ))
    }
}

