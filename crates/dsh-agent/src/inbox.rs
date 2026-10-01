use std::collections::VecDeque;

/// An item waiting in the agent inbox.
#[derive(Debug, Clone, PartialEq)]
pub enum InboxItem {
    /// A regular user message or prompt.
    UserPrompt(String),
    /// Injected context (e.g. system notifications, environment checks, skill definitions).
    InjectedContext { source: String, content: String },
    /// Steering direction amending an ongoing task.
    Steer(String),
}

/// Inbox manager that holds incoming messages before admission into turns.
#[derive(Debug, Default)]
pub struct Inbox {
    queue: VecDeque<InboxItem>,
}

impl Inbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_prompt(&mut self, prompt: impl Into<String>) {
        self.queue.push_back(InboxItem::UserPrompt(prompt.into()));
    }

    pub fn push_injection(&mut self, source: impl Into<String>, content: impl Into<String>) {
        self.queue.push_back(InboxItem::InjectedContext {
            source: source.into(),
            content: content.into(),
        });
    }

    pub fn claim_next(&mut self) -> Option<InboxItem> {
        self.queue.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

