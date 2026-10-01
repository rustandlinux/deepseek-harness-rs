use crate::error::Result;
use async_trait::async_trait;
use dsh_session::SessionEventRecord;
use dsh_types::SessionId;
use serde::{Deserialize, Serialize};

/// 会话基础摘要元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: SessionId,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub event_count: usize,
}

/// 会话持久化底层存储抽象 Trait
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// 创建新会话存储上下文
    async fn create_session(&self, id: &SessionId) -> Result<()>;

    /// 向指定会话批量追加事件记录（保证原子性与单调性）
    async fn append_events(&self, id: &SessionId, records: &[SessionEventRecord]) -> Result<()>;

    /// 读取指定会话的全量持久化事件记录
    async fn load_events(&self, id: &SessionId) -> Result<Vec<SessionEventRecord>>;

    /// 查询并列出所有已知会话的元信息概要
    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>>;

    /// 删除指定会话及其所有关联文件与数据
    async fn delete_session(&self, id: &SessionId) -> Result<()>;
}

