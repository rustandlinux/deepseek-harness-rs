use dsh_llm::{BlockAssembler, LlmAdapter, OpenAiAdapter, StreamChunk};
use dsh_types::Message;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Live Interaction with DeepSeek API ===");
    let adapter = OpenAiAdapter::from_env();

    let messages = vec![
        Message::system("You are an intelligent agent core running on DeepSeek Harness Rust."),
        Message::user("Please introduce yourself and verify connection in 2 concise sentences."),
    ];

    println!("Sending streaming request...");
    let mut stream = adapter.stream(&messages, None);
    let mut assembler = BlockAssembler::new();

    let mut printed_reasoning_header = false;
    let mut printed_content_header = false;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res?;
        match &chunk {
            StreamChunk::ReasoningDelta(delta) => {
                if !printed_reasoning_header {
                    println!("
[Thinking / Reasoning]:");
                    printed_reasoning_header = true;
                }
                print!("{}", delta);
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
            StreamChunk::TextDelta(delta) => {
                if !printed_content_header {
                    println!("
[Assistant Output]:");
                    printed_content_header = true;
                }
                print!("{}", delta);
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
            StreamChunk::Usage(usage) => {
                println!("

[Token Usage]: {:?}", usage);
            }
            StreamChunk::Done => {
                println!("
[Stream Done]");
            }
            _ => {}
        }
        assembler.push_chunk(chunk);
    }

    let (blocks, usage) = assembler.finish();
    println!("
=== BlockAssembler Result ===");
    println!("Assembled {} blocks. Usage: {:?}", blocks.len(), usage);
    for (i, block) in blocks.iter().enumerate() {
        println!("Block {}: {:?}", i, block);
    }

    println!("
Connection and live interaction succeeded!");
    Ok(())
}
