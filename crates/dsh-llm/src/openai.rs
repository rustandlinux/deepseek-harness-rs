use crate::adapter::{BoxStream, LlmAdapter};
use crate::chunk::{StreamChunk, Usage};
use crate::error::{LlmError, Result};
use dsh_types::{ContentBlock, Message};
use futures_util::StreamExt;
use serde::Deserialize;
use std::io::IsTerminal;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// OpenAI-compatible provider adapter (supports standard HTTP streaming, DeepSeek API, etc.).
#[derive(Debug, Clone)]
pub struct OpenAiAdapter {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiAdapter {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let mut url = base_url.into();
        if url.ends_with('/') {
            url.pop();
        }
        Self {
            base_url: url,
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Load default configuration from environment variables or fall back to defaults.
    pub fn from_env() -> Self {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com".to_string());
        let api_key = std::env::var("OPENAI_API_KEY")
            .unwrap_or_default();
        let model = std::env::var("MODEL")
            .unwrap_or_else(|_| "deepseek-chat".to_string());

        Self::new(base_url, api_key, model)
    }

    fn serialize_messages(messages: &[Message]) -> Vec<serde_json::Value> {
        let mut result = Vec::new();

        for msg in messages {
            match msg {
                Message::System(sys) => {
                    result.push(serde_json::json!({
                        "role": "system",
                        "content": sys.content,
                    }));
                }
                Message::User(usr) => {
                    let text = usr
                        .content
                        .iter()
                        .filter_map(|b| b.as_text())
                        .collect::<Vec<_>>()
                        .join("
");
                    result.push(serde_json::json!({
                        "role": "user",
                        "content": text,
                    }));
                }
                Message::Assistant(ast) => {
                    let mut text = String::new();
                    let mut tool_calls = Vec::new();

                    for block in &ast.content {
                        match block {
                            ContentBlock::Text { text: t } => text.push_str(t),
                            ContentBlock::ToolCall { id, name, arguments } => {
                                tool_calls.push(serde_json::json!({
                                    "id": id.as_str(),
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": arguments.to_string(),
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut obj = serde_json::json!({ "role": "assistant" });
                    if !text.is_empty() {
                        obj["content"] = serde_json::Value::String(text);
                    }
                    if !tool_calls.is_empty() {
                        obj["tool_calls"] = serde_json::Value::Array(tool_calls);
                    }
                    result.push(obj);
                }
                Message::Tool(tool) => {
                    result.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool.tool_call_id.as_str(),
                        "content": tool.content,
                    }));
                }
            }
        }

        result
    }
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    index: usize,
    id: Option<String>,
    function: Option<ChunkFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    #[allow(dead_code)]
    index: usize,
    delta: ChunkDelta,
}

#[derive(Debug, Deserialize)]
struct ChunkUsageDetails {
    reasoning_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChunkPromptDetails {
    cached_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChunkUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    completion_tokens_details: Option<ChunkUsageDetails>,
    prompt_tokens_details: Option<ChunkPromptDetails>,
    prompt_cache_hit_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Option<Vec<ChunkChoice>>,
    usage: Option<ChunkUsage>,
}

impl LlmAdapter for OpenAiAdapter {
    fn stream<'a>(
        &'a self,
        messages: &'a [Message],
        tools: Option<&'a [serde_json::Value]>,
    ) -> BoxStream<'a, Result<StreamChunk>> {
        let (tx, rx) = mpsc::channel(128);

        let endpoint = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let serialized_messages = Self::serialize_messages(messages);

        let mut payload = serde_json::json!({
            "model": self.model,
            "messages": serialized_messages,
            "stream": true,
            "stream_options": { "include_usage": true }
        });

        if let Some(t) = tools {
            if !t.is_empty() {
                payload["tools"] = serde_json::Value::Array(t.to_vec());
            }
        }

        let tool_names: Vec<String> = tools
            .map(|t| {
                t.iter()
                    .filter_map(|item| item.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let mut concise_payload = serde_json::json!({
            "model": self.model,
            "messages": serialized_messages,
        });
        if !tool_names.is_empty() {
            concise_payload["tools"] = serde_json::json!(format!("[{} tools omitted: {}]", tool_names.len(), tool_names.join(", ")));
        }

        let client = self.client.clone();
        let model_name = self.model.clone();

        tokio::spawn(async move {
            tracing::debug!(
                target: "dsh::llm::http",
                endpoint = %endpoint,
                model = %model_name,
                "--> 发起 LLM 流式请求"
            );

            if tracing::enabled!(target: "dsh::llm::payload", tracing::Level::DEBUG) {
                if let Ok(pretty) = serde_json::to_string_pretty(&concise_payload) {
                    tracing::debug!(target: "dsh::llm::payload", "请求 Payload (精简):\n{}", pretty);
                }
            }

            if tracing::enabled!(target: "dsh::llm::payload", tracing::Level::TRACE) {
                if let Ok(pretty) = serde_json::to_string_pretty(&payload) {
                    tracing::trace!(target: "dsh::llm::payload", "完整请求 Payload (Trace):\n{}", pretty);
                }
            }

            let is_tty = std::io::stderr().is_terminal();
            let (spinner_tx, mut spinner_rx) = tokio::sync::oneshot::channel::<()>();

            let spinner_handle = if is_tty {
                Some(tokio::spawn(async move {
                    use std::io::Write;
                    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let mut i = 0;
                    let start = std::time::Instant::now();

                    // 120ms 防闪烁缓冲，极快响应不打扰
                    tokio::select! {
                        _ = &mut spinner_rx => return,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(120)) => {}
                    }

                    loop {
                        let elapsed = start.elapsed().as_secs_f32();
                        let frame = frames[i % frames.len()];
                        i += 1;

                        eprint!(
                            "\r\x1b[2K\x1b[38;5;38m{}\x1b[0m \x1b[38;5;252m等待 DeepSeek 响应中...\x1b[0m \x1b[38;5;244m[{:.1}s]\x1b[0m",
                            frame, elapsed
                        );
                        let _ = std::io::stderr().flush();

                        tokio::select! {
                            _ = &mut spinner_rx => break,
                            _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {}
                        }
                    }

                    eprint!("\r\x1b[2K");
                    let _ = std::io::stderr().flush();
                }))
            } else {
                None
            };

            let start_time = std::time::Instant::now();
            let res = client
                .post(&endpoint)
                .bearer_auth(api_key)
                .json(&payload)
                .send()
                .await;

            let elapsed_time = start_time.elapsed();
            let _ = spinner_tx.send(());
            if let Some(handle) = spinner_handle {
                let _ = handle.await;
            }

            let response = match res {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(target: "dsh::llm::http", error = %e, "网络请求失败");
                    let _ = tx.send(Err(LlmError::Network(e))).await;
                    return;
                }
            };

            tracing::debug!(
                target: "dsh::llm::http",
                status = %response.status(),
                elapsed = %format!("{:.2}s", elapsed_time.as_secs_f32()),
                "<-- 收到 HTTP 响应"
            );

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(target: "dsh::llm::http", status = %status, body = %body, "API 返回非成功状态码");
                let _ = tx.send(Err(LlmError::Api { status, body })).await;
                return;
            }

            let mut byte_stream = response.bytes_stream();
            let mut line_buffer = String::new();

            while let Some(chunk_res) = byte_stream.next().await {
                let bytes = match chunk_res {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx.send(Err(LlmError::Network(e))).await;
                        return;
                    }
                };

                let text = match std::str::from_utf8(&bytes) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                line_buffer.push_str(text);

                while let Some(pos) = line_buffer.find('\n') {
                    let line = line_buffer[..pos].trim().to_string();
                    line_buffer = line_buffer[pos + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data:") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            tracing::debug!(target: "dsh::llm::stream", "<-- 收到 [DONE] 流结束信号");
                            let _ = tx.send(Ok(StreamChunk::Done)).await;
                            return;
                        }

                        match serde_json::from_str::<ChatCompletionChunk>(data) {
                            Ok(chunk) => {
                                if let Some(usage) = chunk.usage {
                                    let reasoning_tokens = usage
                                        .completion_tokens_details
                                        .and_then(|d| d.reasoning_tokens);
                                    let prompt_cache_hit = usage
                                        .prompt_cache_hit_tokens
                                        .or_else(|| usage.prompt_tokens_details.and_then(|d| d.cached_tokens));

                                    tracing::debug!(
                                        target: "dsh::llm::usage",
                                        prompt_tokens = usage.prompt_tokens,
                                        completion_tokens = usage.completion_tokens,
                                        prompt_cache_hit_tokens = ?prompt_cache_hit,
                                        reasoning_tokens = ?reasoning_tokens,
                                        "<-- 统计 Token 消耗"
                                    );

                                    let _ = tx
                                        .send(Ok(StreamChunk::Usage(Usage {
                                            prompt_tokens: usage.prompt_tokens,
                                            completion_tokens: usage.completion_tokens,
                                            reasoning_tokens,
                                            prompt_cache_hit_tokens: prompt_cache_hit,
                                        })))
                                        .await;
                                }

                                if let Some(choices) = chunk.choices {
                                    for choice in choices {
                                        if let Some(reasoning) = choice.delta.reasoning_content.or(choice.delta.reasoning) {
                                            if !reasoning.is_empty() {
                                                tracing::trace!(target: "dsh::llm::stream", delta = %reasoning, "Thinking Delta");
                                                let _ = tx.send(Ok(StreamChunk::ReasoningDelta(reasoning))).await;
                                            }
                                        }

                                        if let Some(content) = choice.delta.content {
                                            if !content.is_empty() {
                                                tracing::trace!(target: "dsh::llm::stream", delta = %content, "Text Delta");
                                                let _ = tx.send(Ok(StreamChunk::TextDelta(content))).await;
                                            }
                                        }

                                        if let Some(tool_calls) = choice.delta.tool_calls {
                                            for call in tool_calls {
                                                let func_name = call.function.as_ref().and_then(|f| f.name.clone());
                                                let args_delta = call
                                                    .function
                                                    .as_ref()
                                                    .and_then(|f| f.arguments.clone())
                                                    .unwrap_or_default();

                                                tracing::debug!(
                                                    target: "dsh::llm::tool_call",
                                                    index = call.index,
                                                    id = ?call.id,
                                                    name = ?func_name,
                                                    arguments_delta = %args_delta,
                                                    "<-- 工具调用分片"
                                                );

                                                let _ = tx
                                                    .send(Ok(StreamChunk::ToolCallDelta {
                                                        index: call.index,
                                                        id: call.id,
                                                        name: func_name,
                                                        arguments_delta: args_delta,
                                                    }))
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("failed to parse SSE json chunk: {e}");
                            }
                        }
                    }
                }
            }
        });

        Box::pin(ReceiverStream::new(rx))
    }
}
