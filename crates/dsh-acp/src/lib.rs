pub mod protocol;
pub mod server;

pub use protocol::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
pub use server::AcpServer;

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_llm::{BoxStream, LlmAdapter, StreamChunk};
    use dsh_tools::ToolRegistry;
    use dsh_types::Message;
    use std::sync::Arc;

    struct DummyLlm;
    impl LlmAdapter for DummyLlm {
        fn stream<'a>(
            &'a self,
            _messages: &'a [Message],
            _tools: Option<&'a [serde_json::Value]>,
        ) -> BoxStream<'a, dsh_llm::Result<StreamChunk>> {
            Box::pin(futures_util::stream::iter(vec![
                Ok(StreamChunk::TextDelta("hello from acp".to_string())),
                Ok(StreamChunk::Done),
            ]))
        }
    }

    #[tokio::test]
    async fn test_acp_initialize_and_lifecycle() {
        let mut server = AcpServer::new(Arc::new(DummyLlm), ToolRegistry::new());

        // 1. 测试 initialize
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: serde_json::json!({}),
        };
        let init_resp = server.handle_request(init_req).await.unwrap();
        assert!(init_resp.error.is_none());
        assert_eq!(init_resp.result.unwrap()["protocolVersion"], 1);

        // 2. 测试 session/new
        let new_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "session/new".to_string(),
            params: serde_json::json!({}),
        };
        let new_resp = server.handle_request(new_req).await.unwrap();
        let sid = new_resp.result.unwrap()["sessionId"].as_str().unwrap().to_string();
        assert!(sid.starts_with("sess_"));

        // 3. 测试 session/prompt
        let prompt_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "session/prompt".to_string(),
            params: serde_json::json!({
                "sessionId": sid,
                "prompt": "Hello"
            }),
        };
        let prompt_resp = server.handle_request(prompt_req).await.unwrap();
        assert_eq!(prompt_resp.result.unwrap()["status"], "complete");

        // 4. 测试未知方法错误码 -32601
        let unknown_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(4)),
            method: "non_existent_method".to_string(),
            params: serde_json::json!({}),
        };
        let unknown_resp = server.handle_request(unknown_req).await.unwrap();
        assert_eq!(unknown_resp.error.unwrap().code, -32601);
    }
}

