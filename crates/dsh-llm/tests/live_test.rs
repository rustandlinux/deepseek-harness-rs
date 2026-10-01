use dsh_llm::{BlockAssembler, LlmAdapter, OpenAiAdapter};
use dsh_types::Message;
use futures_util::StreamExt;

#[tokio::test]
async fn test_live_node_interaction() {
    let adapter = OpenAiAdapter::from_env();
    let messages = vec![
        Message::system("You are a test assistant."),
        Message::user("Respond with the single word: PONG"),
    ];

    let mut stream = adapter.stream(&messages, None);
    let mut assembler = BlockAssembler::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Skipping live test or warning: {e}");
                return;
            }
        };
        assembler.push_chunk(chunk);
    }

    let (blocks, usage) = assembler.finish();
    assert!(!blocks.is_empty(), "expected at least one block from live model");
    assert!(usage.is_some(), "expected token usage from live model");

    let text = blocks
        .iter()
        .filter_map(|b| b.as_text())
        .collect::<Vec<_>>()
        .join("");

    assert!(text.to_uppercase().contains("PONG"), "expected PONG in response, got: {}", text);
}

