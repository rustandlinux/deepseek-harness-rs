use crate::error::{Result, ToolError};
use crate::registry::ToolRegistry;
use dsh_types::{ToolCallId, ToolResultMessage};

/// Pipeline orchestrator that coordinates validation, dispatch, and result wrapping.
pub struct ExecutionPipeline {
    registry: ToolRegistry,
}

impl ExecutionPipeline {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    /// Dispatch a tool call through the three-phase pipeline:
    /// pre_execute (validation) -> execute -> post_execute (safe result encapsulation)
    pub async fn dispatch(
        &self,
        call_id: &ToolCallId,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolResultMessage> {
        tracing::debug!(tool = tool_name, call_id = %call_id, "dispatching tool execution");

        // 1. Pre-execute: lookup and resolve tool
        let tool = match self.registry.get(tool_name) {
            Some(t) => t,
            None => {
                return Ok(ToolResultMessage::error(
                    call_id.clone(),
                    format!("Unknown tool: '{}'", tool_name),
                ));
            }
        };

        // 2. Execute with error recovery
        match tool.execute(call_id, args).await {
            Ok(result) => {
                tracing::debug!(tool = tool_name, call_id = %call_id, is_error = result.is_error, "tool execution finished");
                Ok(result)
            }
            Err(ToolError::Timeout { timeout_secs }) => {
                let msg = format!("Tool '{}' execution timed out after {} seconds.", tool_name, timeout_secs);
                Ok(ToolResultMessage::error(call_id.clone(), msg))
            }
            Err(e) => {
                tracing::warn!(tool = tool_name, call_id = %call_id, error = %e, "tool execution returned error");
                Ok(ToolResultMessage::error(call_id.clone(), format!("Tool error: {}", e)))
            }
        }
    }
}

