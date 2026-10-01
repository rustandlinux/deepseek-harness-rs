use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use dsh_agent::ReactLoopAgent;
use dsh_llm::LlmAdapter;
use dsh_tools::ToolRegistry;
use dsh_types::SessionId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

/// ACP 协议服务端核心状态机
pub struct AcpServer {
    llm: Arc<dyn LlmAdapter>,
    tools: ToolRegistry,
    system_prompt: Option<String>,
    active_sessions: HashMap<String, (ReactLoopAgent, CancellationToken)>,
}

impl AcpServer {
    pub fn new(llm: Arc<dyn LlmAdapter>, tools: ToolRegistry) -> Self {
        Self {
            llm,
            tools,
            system_prompt: None,
            active_sessions: HashMap::new(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 处理单条 JSON-RPC 请求并生成对应的响应
    pub async fn handle_request(&mut self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let req_id = match req.id {
            Some(id) => id,
            None => return None, // 通知类消息不生成响应
        };

        match req.method.as_str() {
            "initialize" => {
                let res = serde_json::json!({
                    "protocolVersion": 1,
                    "serverInfo": {
                        "name": "dsh-rust-acp",
                        "version": "0.1.0"
                    },
                    "capabilities": {
                        "streaming": true,
                        "cancellation": true
                    }
                });
                Some(JsonRpcResponse::success(req_id, res))
            }
            "session/new" => {
                let sid = SessionId::generate();
                let mut agent = ReactLoopAgent::new(sid.clone(), self.llm.clone(), self.tools.clone());
                if let Some(ref sys) = self.system_prompt {
                    agent = agent.with_system_prompt(sys);
                }

                let cancel_token = CancellationToken::new();
                self.active_sessions.insert(sid.to_string(), (agent, cancel_token));

                Some(JsonRpcResponse::success(
                    req_id,
                    serde_json::json!({ "sessionId": sid.as_str() }),
                ))
            }
            "session/prompt" => {
                let session_id = match req.params.get("sessionId").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => {
                        return Some(JsonRpcResponse::error(
                            req_id,
                            -32602,
                            "缺失必要参数: 'sessionId'",
                        ));
                    }
                };

                let prompt = match req.params.get("prompt").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => {
                        return Some(JsonRpcResponse::error(
                            req_id,
                            -32602,
                            "缺失必要参数: 'prompt'",
                        ));
                    }
                };

                let session_entry = self.active_sessions.get_mut(session_id);
                match session_entry {
                    Some((agent, cancel_token)) => {
                        let token = cancel_token.clone();
                        match agent.run_turn(prompt, token).await {
                            Ok(reason) => Some(JsonRpcResponse::success(
                                req_id,
                                serde_json::json!({
                                    "status": "complete",
                                    "endReason": format!("{:?}", reason)
                                }),
                            )),
                            Err(e) => Some(JsonRpcResponse::error(
                                req_id,
                                -32000,
                                format!("智能体执行轮次失败: {}", e),
                            )),
                        }
                    }
                    None => Some(JsonRpcResponse::error(
                        req_id,
                        -32001,
                        format!("未找到活动会话: {}", session_id),
                    )),
                }
            }
            "session/cancel" => {
                let session_id = match req.params.get("sessionId").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => {
                        return Some(JsonRpcResponse::error(
                            req_id,
                            -32602,
                            "缺失必要参数: 'sessionId'",
                        ));
                    }
                };

                if let Some((_, cancel_token)) = self.active_sessions.get(session_id) {
                    cancel_token.cancel();
                    Some(JsonRpcResponse::success(
                        req_id,
                        serde_json::json!({ "cancelled": true }),
                    ))
                } else {
                    Some(JsonRpcResponse::error(
                        req_id,
                        -32001,
                        format!("未找到活动会话: {}", session_id),
                    ))
                }
            }
            unknown => Some(JsonRpcResponse::error(
                req_id,
                -32601,
                format!("未支持的 RPC 方法: '{}'", unknown),
            )),
        }
    }

    /// 启动 stdio 通信流，持续处理来自客户端编辑器的请求
    pub async fn run_stdio<R, W>(&mut self, reader: R, mut writer: W) -> std::io::Result<()>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    let err_res = JsonRpcResponse::error(
                        serde_json::Value::Null,
                        -32700,
                        format!("JSON 反序列化错误: {}", e),
                    );
                    let serialized = serde_json::to_string(&err_res)?;
                    writer.write_all(serialized.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }
            };

            if let Some(resp) = self.handle_request(req).await {
                let serialized = serde_json::to_string(&resp)?;
                writer.write_all(serialized.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }

        Ok(())
    }
}

