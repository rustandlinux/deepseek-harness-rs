use crate::error::Result;
use crate::inbox::Inbox;
use dsh_llm::{BlockAssembler, LlmAdapter};
use dsh_session::{derive_messages, Session, SessionEvent, TurnEndReason};
use dsh_tools::{ExecutionPipeline, ToolRegistry};
use dsh_types::{
    AssistantMessage, ContentBlock, SessionId, SystemMessage, UserMessage,
};
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// The primary ReAct agent loop driver.
pub struct ReactLoopAgent {
    session: Arc<Mutex<Session>>,
    llm: Arc<dyn LlmAdapter>,
    pipeline: Arc<ExecutionPipeline>,
    tools: ToolRegistry,
    system_prompt: Option<String>,
    max_steps_per_turn: u32,
    turn_counter: u32,
    pub inbox: Inbox,
}

impl ReactLoopAgent {
    pub fn new(
        session_id: SessionId,
        llm: Arc<dyn LlmAdapter>,
        tools: ToolRegistry,
    ) -> Self {
        let pipeline = Arc::new(ExecutionPipeline::new(tools.clone()));
        Self {
            session: Arc::new(Mutex::new(Session::new(session_id))),
            llm,
            pipeline,
            tools,
            system_prompt: None,
            max_steps_per_turn: 15,
            turn_counter: 0,
            inbox: Inbox::new(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_max_steps(mut self, max_steps: u32) -> Self {
        self.max_steps_per_turn = max_steps;
        self
    }

    pub fn session(&self) -> Arc<Mutex<Session>> {
        self.session.clone()
    }

    /// Run an agent turn for a user prompt until all steps finish or abort.
    pub async fn run_turn(
        &mut self,
        prompt: &str,
        cancel: CancellationToken,
    ) -> Result<TurnEndReason> {
        self.turn_counter += 1;
        let turn = self.turn_counter;

        {
            let mut sess = self.session.lock().await;
            sess.append(SessionEvent::TurnStart { turn });

            if turn == 1 {
                if let Some(ref sys) = self.system_prompt {
                    sess.append(SessionEvent::SystemMessage(SystemMessage::new(sys.clone())));
                }
            }

            sess.append(SessionEvent::UserMessage(UserMessage::text(prompt)));
        }

        let mut step: u32 = 1;

        loop {
            if cancel.is_cancelled() {
                let mut sess = self.session.lock().await;
                sess.append(SessionEvent::TurnEnd {
                    turn,
                    reason: TurnEndReason::Aborted,
                });
                return Ok(TurnEndReason::Aborted);
            }

            if step > self.max_steps_per_turn {
                let mut sess = self.session.lock().await;
                sess.append(SessionEvent::TurnEnd {
                    turn,
                    reason: TurnEndReason::MaxSteps,
                });
                return Ok(TurnEndReason::MaxSteps);
            }

            {
                let mut sess = self.session.lock().await;
                sess.append(SessionEvent::StepStart { turn, step });
            }

            // 1. Derive messages from append-only event log
            let messages = {
                let sess = self.session.lock().await;
                derive_messages(sess.records())
            };

            let tool_schemas = self.tools.export_openai_tools();

            // 2. Stream LLM call
            let mut stream = self.llm.stream(&messages, Some(&tool_schemas));
            let mut assembler = BlockAssembler::new();

            while let Some(chunk_res) = stream.next().await {
                if cancel.is_cancelled() {
                    let mut sess = self.session.lock().await;
                    sess.append(SessionEvent::StepEnd { turn, step });
                    sess.append(SessionEvent::TurnEnd {
                        turn,
                        reason: TurnEndReason::Aborted,
                    });
                    return Ok(TurnEndReason::Aborted);
                }

                let chunk = chunk_res?;
                assembler.push_chunk(chunk);
            }

            let (blocks, _usage) = assembler.finish();

            // 3. Commit assistant message to log
            {
                let mut sess = self.session.lock().await;
                sess.append(SessionEvent::AssistantMessage(AssistantMessage::new(
                    blocks.clone(),
                )));
            }

            // 4. Identify any requested tool calls
            let mut tool_calls = Vec::new();
            for block in &blocks {
                if let ContentBlock::ToolCall { id, name, arguments } = block {
                    tool_calls.push((id.clone(), name.clone(), arguments.clone()));
                }
            }

            // 5. If no tools were called, the turn is complete!
            if tool_calls.is_empty() {
                let mut sess = self.session.lock().await;
                sess.append(SessionEvent::StepEnd { turn, step });
                sess.append(SessionEvent::TurnEnd {
                    turn,
                    reason: TurnEndReason::Complete,
                });
                return Ok(TurnEndReason::Complete);
            }

            // 6. Execute requested tools and record events
            for (id, name, args) in tool_calls {
                {
                    let mut sess = self.session.lock().await;
                    sess.append(SessionEvent::ToolCall {
                        turn,
                        step,
                        call: ContentBlock::tool_call(id.clone(), name.clone(), args.clone()),
                    });
                }

                let result = self.pipeline.dispatch(&id, &name, args).await?;

                {
                    let mut sess = self.session.lock().await;
                    sess.append(SessionEvent::ToolResult {
                        turn,
                        step,
                        result,
                    });
                }
            }

            {
                let mut sess = self.session.lock().await;
                sess.append(SessionEvent::StepEnd { turn, step });
            }

            step += 1;
        }
    }
}

