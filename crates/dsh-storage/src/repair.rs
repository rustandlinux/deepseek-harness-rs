use dsh_session::{SessionEvent, SessionEventRecord, TurnEndReason};
use dsh_types::{ContentBlock, SessionSeq, ToolCallId, ToolResultMessage};
use std::collections::HashMap;

/// 对从存储层加载的原始事件记录进行崩溃自愈与因果链对齐
///
/// 若进程在执行过程中异常崩溃，导致产生悬空的 Step、未闭合的 Turn 或未返回结果的 ToolCall，
/// 本函数将自动合成缺失的对偶事件并保证序列号绝对单调递增，避免重放断层。
pub fn repair_session_records(mut records: Vec<SessionEventRecord>) -> Vec<SessionEventRecord> {
    if records.is_empty() {
        return records;
    }

    let mut current_open_turn: Option<u32> = None;
    let mut current_open_step: Option<(u32, u32)> = None;
    let mut pending_tools: HashMap<ToolCallId, (u32, u32)> = HashMap::new();

    for record in &records {
        match &record.event {
            SessionEvent::TurnStart { turn } => {
                current_open_turn = Some(*turn);
            }
            SessionEvent::TurnEnd { .. } => {
                current_open_turn = None;
            }
            SessionEvent::StepStart { turn, step } => {
                current_open_step = Some((*turn, *step));
            }
            SessionEvent::StepEnd { .. } => {
                current_open_step = None;
            }
            SessionEvent::ToolCall { turn, step, call } => {
                if let ContentBlock::ToolCall { id, .. } = call {
                    pending_tools.insert(id.clone(), (*turn, *step));
                }
            }
            SessionEvent::ToolResult { result, .. } => {
                pending_tools.remove(&result.tool_call_id);
            }
            _ => {}
        }
    }

    // 若无未闭合的悬空状态，直接返回原列表
    if current_open_turn.is_none() && current_open_step.is_none() && pending_tools.is_empty() {
        return records;
    }

    let mut next_seq = records.last().map(|r| r.seq.next()).unwrap_or(SessionSeq::ZERO);

    // 1. 为所有未收到结果的工具调用补充合成错误事件
    for (tool_id, (turn, step)) in pending_tools {
        let synthetic_result = ToolResultMessage::error(
            tool_id,
            "工具执行因宿主进程意外中断而中止，由持久化崩溃恢复引擎自动补齐错误响应。",
        );
        records.push(SessionEventRecord::new(
            next_seq,
            SessionEvent::ToolResult {
                turn,
                step,
                result: synthetic_result,
            },
        ));
        next_seq = next_seq.next();
    }

    // 2. 闭合未完成的步进（Step）
    if let Some((turn, step)) = current_open_step {
        records.push(SessionEventRecord::new(
            next_seq,
            SessionEvent::StepEnd { turn, step },
        ));
        next_seq = next_seq.next();
    }

    // 3. 闭合未完成的轮次（Turn）
    if let Some(turn) = current_open_turn {
        records.push(SessionEventRecord::new(
            next_seq,
            SessionEvent::TurnEnd {
                turn,
                reason: TurnEndReason::Error(
                    "会话所在进程异常退出，轮次已由崩溃恢复机制安全闭合。".to_string(),
                ),
            },
        ));
    }

    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_types::UserMessage;

    #[test]
    fn test_repair_unclosed_turn_and_step() {
        let records = vec![
            SessionEventRecord::new(0.into(), SessionEvent::TurnStart { turn: 1 }),
            SessionEventRecord::new(1.into(), SessionEvent::StepStart { turn: 1, step: 1 }),
            SessionEventRecord::new(2.into(), SessionEvent::UserMessage(UserMessage::text("test"))),
            SessionEventRecord::new(
                3.into(),
                SessionEvent::ToolCall {
                    turn: 1,
                    step: 1,
                    call: ContentBlock::tool_call("call_crash", "bash", serde_json::json!({})),
                },
            ),
        ];

        let repaired = repair_session_records(records);
        assert_eq!(repaired.len(), 7); // 原有4条 + 合成ToolResult + 合成StepEnd + 合成TurnEnd

        assert_eq!(repaired[4].seq.as_u64(), 4);
        assert!(matches!(
            &repaired[4].event,
            SessionEvent::ToolResult { result, .. } if result.tool_call_id.as_str() == "call_crash" && result.is_error
        ));

        assert_eq!(repaired[5].seq.as_u64(), 5);
        assert!(matches!(&repaired[5].event, SessionEvent::StepEnd { .. }));

        assert_eq!(repaired[6].seq.as_u64(), 6);
        assert!(matches!(
            &repaired[6].event,
            SessionEvent::TurnEnd { reason: TurnEndReason::Error(_), .. }
        ));
    }
}

