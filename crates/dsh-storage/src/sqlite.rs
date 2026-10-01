use crate::error::{Result, StorageError};
use crate::repair::repair_session_records;
use crate::store::{SessionMetadata, SessionStore};
use async_trait::async_trait;
use chrono::Utc;
use dsh_session::SessionEventRecord;
use dsh_types::SessionId;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 基于 SQLite 关系型数据库与 WAL 模式的高可靠会话持久化存储驱动
#[derive(Clone)]
pub struct SqliteSessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSessionStore {
    /// 打开或新建本地 SQLite 数据库文件并初始化表结构
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init_connection(conn)
    }

    /// 打开内存 SQLite 数据库（主要用于测试与高频单元验证）
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_connection(conn)
    }

    fn init_connection(conn: Connection) -> Result<Self> {
        // 启用 WAL 模式以提升并发读写吞吐
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        // 初始化会话元数据表与事件记录表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session_events (
                session_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                payload TEXT NOT NULL,
                PRIMARY KEY (session_id, seq),
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_session_events_seq 
            ON session_events (session_id, seq ASC);",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create_session(&self, id: &SessionId) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3)",
            params![id.as_str(), now, now],
        )?;
        Ok(())
    }

    async fn append_events(&self, id: &SessionId, records: &[SessionEventRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().await;
        let now = Utc::now().timestamp_millis();

        let tx = conn.transaction()?;
        {
            // 确保会话主记录存在
            tx.execute(
                "INSERT OR IGNORE INTO sessions (id, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3)",
                params![id.as_str(), now, now],
            )?;

            // 批量插入事件
            let mut stmt = tx.prepare(
                "INSERT INTO session_events (session_id, seq, timestamp_ms, payload) VALUES (?1, ?2, ?3, ?4)"
            )?;

            for record in records {
                let payload = serde_json::to_string(record)?;
                stmt.execute(params![
                    id.as_str(),
                    record.seq.as_u64() as i64,
                    record.timestamp_ms,
                    payload
                ])?;
            }

            // 更新会话更新时间
            tx.execute(
                "UPDATE sessions SET updated_at_ms = ?1 WHERE id = ?2",
                params![now, id.as_str()],
            )?;
        }
        tx.commit()?;

        Ok(())
    }

    async fn load_events(&self, id: &SessionId) -> Result<Vec<SessionEventRecord>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT payload FROM session_events WHERE session_id = ?1 ORDER BY seq ASC"
        )?;

        let mut rows = stmt.query(params![id.as_str()])?;
        let mut raw_records = Vec::new();

        while let Some(row) = rows.next()? {
            let payload: String = row.get(0)?;
            let record: SessionEventRecord = serde_json::from_str(&payload)?;
            raw_records.push(record);
        }

        if raw_records.is_empty() {
            // 检查会话是否存在
            let mut check_stmt = conn.prepare("SELECT 1 FROM sessions WHERE id = ?1")?;
            let exists = check_stmt.exists(params![id.as_str()])?;
            if !exists {
                return Err(StorageError::SessionNotFound(id.to_string()));
            }
        }

        Ok(repair_session_records(raw_records))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT s.id, s.created_at_ms, s.updated_at_ms, COUNT(e.seq) as event_count
             FROM sessions s
             LEFT JOIN session_events e ON s.id = e.session_id
             GROUP BY s.id
             ORDER BY s.created_at_ms ASC"
        )?;

        let mut rows = stmt.query([])?;
        let mut list = Vec::new();

        while let Some(row) = rows.next()? {
            let raw_id: String = row.get(0)?;
            let created_at_ms: i64 = row.get(1)?;
            let updated_at_ms: i64 = row.get(2)?;
            let event_count: i64 = row.get(3)?;

            list.push(SessionMetadata {
                id: SessionId::new(raw_id),
                created_at_ms,
                updated_at_ms,
                event_count: event_count as usize,
            });
        }

        Ok(list)
    }

    async fn delete_session(&self, id: &SessionId) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id.as_str()])?;
        Ok(())
    }
}

