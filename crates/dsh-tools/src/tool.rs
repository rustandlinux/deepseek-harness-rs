use crate::error::Result;
use async_trait::async_trait;
use dsh_types::{ToolCallId, ToolResultMessage};
use serde::{Deserialize, Serialize};

/// Metadata schema describing a tool for model consumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    /// Export as an OpenAI-compatible function definition format.
    pub fn to_openai_value(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        })
    }
}

/// Abstract contract for an executable tool in the harness.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique identifier/name of the tool (e.g. "bash", "read_file").
    fn name(&self) -> &str;

    /// High-level description of what the tool accomplishes.
    fn description(&self) -> &str;

    /// JSON schema describing the required and optional parameters.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Generate the full schema descriptor.
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }

    /// Execute the tool with the given invocation ID and JSON parameters.
    async fn execute(&self, call_id: &ToolCallId, args: serde_json::Value) -> Result<ToolResultMessage>;
}

