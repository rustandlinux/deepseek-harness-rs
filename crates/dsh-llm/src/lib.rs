pub mod adapter;
pub mod assembler;
pub mod chunk;
pub mod error;
pub mod openai;

pub use adapter::{BoxStream, LlmAdapter};
pub use assembler::BlockAssembler;
pub use chunk::{StreamChunk, Usage};
pub use error::{LlmError, Result};
pub use openai::OpenAiAdapter;

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_types::ContentBlock;

    #[test]
    fn test_assembler_text_and_reasoning() {
        let mut assembler = BlockAssembler::new();

        assembler.push_chunk(StreamChunk::ReasoningDelta("step 1: analyze\n".into()));
        assembler.push_chunk(StreamChunk::ReasoningDelta("step 2: conclude".into()));
        assembler.push_chunk(StreamChunk::TextDelta("Here is ".into()));
        assembler.push_chunk(StreamChunk::TextDelta("the answer.".into()));
        assembler.push_chunk(StreamChunk::Usage(Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            reasoning_tokens: Some(8),
            prompt_cache_hit_tokens: None,
        }));
        assembler.push_chunk(StreamChunk::Done);

        let (blocks, usage) = assembler.finish();
        assert_eq!(blocks.len(), 2);

        match &blocks[0] {
            ContentBlock::Reasoning { reasoning } => {
                assert_eq!(reasoning, "step 1: analyze\nstep 2: conclude");
            }
            _ => panic!("expected reasoning block"),
        }

        match &blocks[1] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "Here is the answer.");
            }
            _ => panic!("expected text block"),
        }

        assert!(usage.is_some());
        assert_eq!(usage.unwrap().reasoning_tokens, Some(8));
    }

    #[test]
    fn test_assembler_tool_calls() {
        let mut assembler = BlockAssembler::new();

        assembler.push_chunk(StreamChunk::ToolCallDelta {
            index: 0,
            id: Some("call_abc".into()),
            name: Some("bash".into()),
            arguments_delta: "{\"cmd\":".into(),
        });
        assembler.push_chunk(StreamChunk::ToolCallDelta {
            index: 0,
            id: None,
            name: None,
            arguments_delta: " \"cargo check\"}".into(),
        });

        let (blocks, _) = assembler.finish();
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            ContentBlock::ToolCall { id, name, arguments } => {
                assert_eq!(id.as_str(), "call_abc");
                assert_eq!(name, "bash");
                assert_eq!(arguments["cmd"], "cargo check");
            }
            _ => panic!("expected tool call block"),
        }
    }
}

