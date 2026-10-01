use clap::{Parser, Subcommand};
use dsh_acp::AcpServer;
use dsh_agent::ReactLoopAgent;
use dsh_llm::OpenAiAdapter;
use dsh_session::{SessionEvent, TurnEndReason};
use dsh_storage::{JsonlSessionStore, SessionStore};
use dsh_tools::{BashTool, FileReadTool, FileWriteTool, ToolRegistry};
use dsh_types::{ContentBlock, SessionId};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{stdin, stdout, AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

/// 自定义终端格式化器：针对暗色终端优化日志级别颜色，DEBUG采用温和清晰的柔和青色
struct DshConsoleFormatter;

impl<S, N> FormatEvent<S, N> for DshConsoleFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let meta = event.metadata();
        let level_str = match *meta.level() {
            Level::ERROR => "\x1b[1;31mERROR\x1b[0m",
            Level::WARN => "\x1b[1;33m WARN\x1b[0m",
            Level::INFO => "\x1b[1;32m INFO\x1b[0m",
            Level::DEBUG => "\x1b[38;5;38mDEBUG\x1b[0m", // 柔和舒适的青色（Cyan），暗色背景下清晰柔和、不刺眼
            Level::TRACE => "\x1b[38;5;141mTRACE\x1b[0m",
        };
        write!(writer, "{} \x1b[38;5;244m{}\x1b[0m: ", level_str, meta.target())?;
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

#[derive(Parser)]
#[command(
    name = "dsh",
    about = "DeepSeek Harness (Rust 原生实现) 统一调度与交互命令行工具",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 执行一次性命令任务 (Headless 模式)
    Run {
        /// 用户的自然语言任务描述
        task: String,
        /// 可选的系统提示词
        #[arg(short, long)]
        system: Option<String>,
        /// 持久化存储目录 (默认: .dsh/sessions)
        #[arg(long, default_value = ".dsh/sessions")]
        storage_dir: PathBuf,
    },
    /// 启动交互式终端对话 (Chat REPL 模式)
    Chat {
        /// 可选的系统提示词
        #[arg(short, long)]
        system: Option<String>,
        /// 持久化存储目录 (默认: .dsh/sessions)
        #[arg(long, default_value = ".dsh/sessions")]
        storage_dir: PathBuf,
    },
    /// 启动标准输入输出的 ACP (Agent Client Protocol) 协议服务
    Acp {
        /// 可选的系统提示词
        #[arg(short, long)]
        system: Option<String>,
    },
    /// 查询并列出所有持久化保存的本地会话
    Sessions {
        /// 持久化存储目录 (默认: .dsh/sessions)
        #[arg(long, default_value = ".dsh/sessions")]
        storage_dir: PathBuf,
    },
}

fn build_default_tools() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(BashTool::new());
    registry.register(FileReadTool::new());
    registry.register(FileWriteTool);
    registry
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default_filter = if cfg!(debug_assertions) {
        "warn,dsh=debug,dsh_llm=debug,dsh_agent=debug,dsh_tools=debug,dsh_session=debug,dsh_storage=debug,dsh_acp=debug"
    } else {
        "warn"
    };

    let env_filter = tracing_subscriber::EnvFilter::builder()
        .parse_lossy(
            std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_string()),
        );

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(DshConsoleFormatter)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            task,
            system,
            storage_dir,
        } => {
            println!("🚀 启动一次性任务 (Headless 模式)...");
            let adapter = Arc::new(OpenAiAdapter::from_env());
            let tools = build_default_tools();
            let session_id = SessionId::generate();

            let sys_prompt = system.unwrap_or_else(|| {
                "You are an autonomous coding assistant running on DeepSeek Harness Rust. Use tools to solve tasks.".to_string()
            });

            let mut agent = ReactLoopAgent::new(session_id.clone(), adapter, tools)
                .with_system_prompt(sys_prompt);

            println!("会话 ID: {}", session_id);
            println!("执行任务: {}", task);
            println!("---\n");

            let cancel = CancellationToken::new();
            let reason = agent.run_turn(&task, cancel).await?;

            // 获取并展示最终生成的输出
            let session = agent.session();
            let sess = session.lock().await;

            let store = JsonlSessionStore::new(&storage_dir);
            store.create_session(&session_id).await?;
            store.append_events(&session_id, sess.records()).await?;

            println!("\n--- 任务完成 ---");
            println!("终止原因: {:?}", reason);

            if let Some(last_assistant) = sess.records().iter().rev().find_map(|r| match &r.event {
                SessionEvent::AssistantMessage(msg) => Some(msg),
                _ => None,
            }) {
                for block in &last_assistant.content {
                    if let ContentBlock::Text { text } = block {
                        println!("\n[输出回答]:\n{}", text);
                    }
                }
            }

            println!("\n会话记录已持久化至: {}/{}.jsonl", storage_dir.display(), session_id);
        }
        Commands::Chat {
            system,
            storage_dir,
        } => {
            println!("======================================================");
            println!("   DeepSeek Harness (Rust 原生版本) 交互式终端 (REPL)   ");
            println!("======================================================");
            println!("输入任务指令开始对话，输入 'exit' 或 'quit' 退出。\n");

            let adapter = Arc::new(OpenAiAdapter::from_env());
            let tools = build_default_tools();
            let session_id = SessionId::generate();

            let sys_prompt = system.unwrap_or_else(|| {
                "You are a helpful coding assistant running on DeepSeek Harness Rust.".to_string()
            });

            let mut agent = ReactLoopAgent::new(session_id.clone(), adapter, tools)
                .with_system_prompt(sys_prompt);

            let store = JsonlSessionStore::new(&storage_dir);
            store.create_session(&session_id).await?;

            let mut reader = BufReader::new(stdin()).lines();

            loop {
                print!("dsh> ");
                std::io::stdout().flush()?;

                let line = match reader.next_line().await? {
                    Some(l) => l.trim().to_string(),
                    None => break,
                };

                if line.is_empty() {
                    continue;
                }

                if line == "exit" || line == "quit" {
                    println!("退出对话。");
                    break;
                }

                println!("🤖 思考中...");
                let cancel = CancellationToken::new();
                let reason = agent.run_turn(&line, cancel).await?;

                let session = agent.session();
                let sess = session.lock().await;

                // 持久化最新增量
                store.append_events(&session_id, sess.records()).await?;

                if let Some(last_assistant) = sess.records().iter().rev().find_map(|r| match &r.event {
                    SessionEvent::AssistantMessage(msg) => Some(msg),
                    _ => None,
                }) {
                    for block in &last_assistant.content {
                        match block {
                            ContentBlock::Text { text } => {
                                println!("\n{}\n", text);
                            }
                            ContentBlock::ToolCall { name, arguments, .. } => {
                                println!("🔧 [调用工具]: {} ({})", name, arguments);
                            }
                            _ => {}
                        }
                    }
                }

                if reason != TurnEndReason::Complete {
                    println!("⚠️ [轮次状态]: {:?}", reason);
                }
            }
        }
        Commands::Acp { system } => {
            let adapter = Arc::new(OpenAiAdapter::from_env());
            let tools = build_default_tools();

            let mut server = AcpServer::new(adapter, tools);
            if let Some(sys) = system {
                server = server.with_system_prompt(sys);
            }

            let reader = BufReader::new(stdin());
            let writer = stdout();
            server.run_stdio(reader, writer).await?;
        }
        Commands::Sessions { storage_dir } => {
            let store = JsonlSessionStore::new(&storage_dir);
            let sessions = store.list_sessions().await?;

            println!("=== 本地存储会话列表 ({}) ===", storage_dir.display());
            if sessions.is_empty() {
                println!("暂无持久化会话记录。");
            } else {
                for (i, meta) in sessions.iter().enumerate() {
                    println!(
                        "[{}] 会话 ID: {} | 事件数: {} | 更新时间: {}",
                        i + 1,
                        meta.id,
                        meta.event_count,
                        meta.updated_at_ms
                    );
                }
            }
        }
    }

    Ok(())
}
