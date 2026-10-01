use dsh_session::{Session, SessionEvent, SessionEventRecord, TurnEndReason};
use dsh_storage::{JsonlSessionStore, SessionStore, SqliteSessionStore};
use dsh_types::{ContentBlock, SessionId, UserMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 会话持久化与崩溃自愈引擎演示 ===");

    // 1. 测试 JSONL 文件持久化驱动
    println!("\n[1] 验证 JSONL 文件持久化...");
    let jsonl_dir = std::env::temp_dir().join("dsh_example_jsonl");
    let jsonl_store = JsonlSessionStore::new(&jsonl_dir);

    let sid1 = SessionId::generate();
    jsonl_store.create_session(&sid1).await?;

    let records1 = vec![
        SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
        SessionEventRecord::new(1.into(), SessionEvent::UserMessage(UserMessage::text("JSONL 会话测试"))),
        SessionEventRecord::new(2.into(), SessionEvent::TurnEnd { turn: 1, reason: TurnEndReason::Complete }),
    ];
    jsonl_store.append_events(&sid1, &records1).await?;
    let loaded1 = jsonl_store.load_events(&sid1).await?;
    println!("JSONL 成功加载 {} 条事件记录，第一条为 {:?}", loaded1.len(), loaded1[1].event);

    // 2. 测试 SQLite 驱动与 WAL 事务
    println!("\n[2] 验证 SQLite 持久化驱动...");
    let db_path = std::env::temp_dir().join("dsh_example_sqlite.db");
    let sqlite_store = SqliteSessionStore::open(&db_path)?;

    let sid2 = SessionId::generate();
    sqlite_store.create_session(&sid2).await?;

    let records2 = vec![
        SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
        SessionEventRecord::new(1.into(), SessionEvent::UserMessage(UserMessage::text("SQLite 会话测试"))),
        SessionEventRecord::new(2.into(), SessionEvent::TurnEnd { turn: 1, reason: TurnEndReason::Complete }),
    ];
    sqlite_store.append_events(&sid2, &records2).await?;
    let loaded2 = sqlite_store.load_events(&sid2).await?;
    println!("SQLite 成功加载 {} 条事件记录", loaded2.len());

    // 3. 模拟进程异常崩溃与自动修复自愈
    println!("\n[3] 模拟进程崩溃场景与自动修复...");
    let sid3 = SessionId::generate();
    sqlite_store.create_session(&sid3).await?;

    // 故意写入一个因为崩溃而中断的会话日志：
    // 发起了 turn 1、step 1，并发出了 bash 工具调用，但程序立即意外退出，没有后续 step/end 和 turn/end
    let crashed_records = vec![
        SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
        SessionEventRecord::new(1.into(), SessionEvent::StepStart { turn: 1, step: 1 }),
        SessionEventRecord::new(
            2.into(),
            SessionEvent::ToolCall {
                turn: 1,
                step: 1,
                call: ContentBlock::tool_call("call_crash_001", "bash", serde_json::json!({ "cmd": "sleep 10" })),
            },
        ),
    ];
    sqlite_store.append_events(&sid3, &crashed_records).await?;
    println!("写入模拟崩溃日志（仅 3 条记录，存在未决 ToolCall 与未闭合 Turn）...");

    // 读取该会话，引擎将自动触发 repair_session_records 修复
    let repaired = sqlite_store.load_events(&sid3).await?;
    println!("恢复引擎读取并完成自愈修复！修复后事件总数: {} 条", repaired.len());

    for record in &repaired {
        println!("  [seq: {}] {:?}", record.seq, record.event);
    }

    // 验证修复后的数据能够直接重新构造出合法合规的 Session 领域对象
    let session = Session::from_records(sid3, repaired)?;
    session.validate_continuity()?;
    println!("\n验证通过：自愈修复后的事件流完全连续且自洽！");

    println!("\n=== 会话持久化与崩溃自愈引擎验证成功 ===");
    Ok(())
}

