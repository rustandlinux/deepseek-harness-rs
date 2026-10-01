use crate::chunk::{StreamChunk, Usage};
use dsh_types::{ContentBlock, ToolCallId};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
struct InProgressToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments_buf: String,
}

/// Stateful assembler that accumulates incremental StreamChunks into structured ContentBlocks.
#[derive(Debug, Default)]
pub struct BlockAssembler {
    reasoning_buf: String,
    text_buf: String,
    tool_calls: BTreeMap<usize, InProgressToolCall>,
    usage: Option<Usage>,
}

impl BlockAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a stream chunk.
    pub fn push_chunk(&mut self, chunk: StreamChunk) {
        match chunk {
            StreamChunk::ReasoningDelta(delta) => {
                self.reasoning_buf.push_str(&delta);
            }
            StreamChunk::TextDelta(delta) => {
                self.text_buf.push_str(&delta);
            }
            StreamChunk::ToolCallDelta {
                index,
                id,
                name,
                arguments_delta,
            } => {
                let entry = self.tool_calls.entry(index).or_default();
                if let Some(call_id) = id {
                    entry.id = Some(call_id);
                }
                if let Some(tool_name) = name {
                    entry.name = Some(tool_name);
                }
                entry.arguments_buf.push_str(&arguments_delta);
            }
            StreamChunk::Usage(usage) => {
                self.usage = Some(usage);
            }
            StreamChunk::Done => {}
        }
    }

    /// Complete assembly and produce the ordered list of finalized ContentBlocks.
    pub fn finish(self) -> (Vec<ContentBlock>, Option<Usage>) {
        let mut blocks = Vec::new();

        // 1. If reasoning was captured, emit ReasoningBlock first
        if !self.reasoning_buf.is_empty() {
            blocks.push(ContentBlock::reasoning(self.reasoning_buf));
        }

        // 2. If main response text was captured, emit TextBlock
        if !self.text_buf.is_empty() {
            blocks.push(ContentBlock::text(self.text_buf));
        }

        // 3. Emit ToolCallBlocks ordered by call index
        for (_idx, tool) in self.tool_calls {
            let id = tool
                .id
                .map(ToolCallId::new)
                .unwrap_or_else(ToolCallId::generate);
            let name = tool.name.unwrap_or_else(|| "unknown_tool".to_string());
            let arguments = match serde_json::from_str::<serde_json::Value>(&tool.arguments_buf) {
                Ok(val) => val,
                Err(_) => serde_json::json!({ "raw": tool.arguments_buf }),
            };
            blocks.push(ContentBlock::tool_call(id, name, arguments));
        }

        (blocks, self.usage)
    }
}

