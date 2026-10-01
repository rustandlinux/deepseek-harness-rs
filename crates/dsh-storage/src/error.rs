use thiserror::Error;

/// 存储层领域错误定义
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("会话 '{0}' 未找到")]
    SessionNotFound(String),

    #[error("IO 读写错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 序列化/反序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SQLite 数据库操作失败: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("数据日志损坏或格式不兼容: {0}")]
    CorruptData(String),

    #[error("会话状态错误: {0}")]
    Session(#[from] dsh_session::SessionError),
}

pub type Result<T> = std::result::Result<T, StorageError>;

