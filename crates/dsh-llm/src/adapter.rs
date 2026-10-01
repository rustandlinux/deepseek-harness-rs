use crate::chunk::StreamChunk;
use crate::error::Result;
use dsh_types::Message;
use futures_util::Stream;
use std::pin::Pin;

pub type BoxStream<'a, T> = Pin<Box<dyn Stream<Item = T> + Send + 'a>>;

/// Trait defining the asynchronous streaming interface for LLM model providers.
pub trait LlmAdapter: Send + Sync {
    /// Initiate a streaming completion for the given conversation history and optional tools.
    fn stream<'a>(
        &'a self,
        messages: &'a [Message],
        tools: Option<&'a [serde_json::Value]>,
    ) -> BoxStream<'a, Result<StreamChunk>>;
}

