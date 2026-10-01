use crate::error::{Result, StorageError};
use crate::repair::repair_session_records;
use crate::store::{SessionMetadata, SessionStore};
use async_trait::async_trait;
use chrono::Utc;
use dsh_session::SessionEventRecord;
use dsh_types::SessionId;
use std::path::PathBuf;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// 基于 JSONL（每行一条 JSON 记录）与 Sidecar 元数据文件的会话持久化存储驱动
pub struct JsonlSessionStore {
    base_dir: PathBuf,
}

impl JsonlSessionStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn session_file_path(&self, id: &SessionId) -> PathBuf {
        self.base_dir.join(format!("{}.jsonl", id.as_str()))
    }

    fn meta_file_path(&self, id: &SessionId) -> PathBuf {
        self.base_dir.join(format!("{}.meta.json", id.as_str()))
    }
}

#[async_trait]
impl SessionStore for JsonlSessionStore {
    async fn create_session(&self, id: &SessionId) -> Result<()> {
        tokio::fs::create_dir_all(&self.base_dir).await?;

        let meta = SessionMetadata {
            id: id.clone(),
            created_at_ms: Utc::now().timestamp_millis(),
            updated_at_ms: Utc::now().timestamp_millis(),
            event_count: 0,
        };

        let meta_json = serde_json::to_string_pretty(&meta)?;
        tokio::fs::write(self.meta_file_path(id), meta_json).await?;

        // 预创建空的 jsonl 文件
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.session_file_path(id))
            .await?;

        Ok(())
    }

    async fn append_events(&self, id: &SessionId, records: &[SessionEventRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        tokio::fs::create_dir_all(&self.base_dir).await?;
        let session_path = self.session_file_path(id);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&session_path)
            .await?;

        for record in records {
            let line = serde_json::to_string(record)?;
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        file.flush().await?;

        // 同步更新 Sidecar 元数据
        let meta_path = self.meta_file_path(id);
        let mut meta = if meta_path.exists() {
            let data = tokio::fs::read_to_string(&meta_path).await?;
            serde_json::from_str::<SessionMetadata>(&data).unwrap_or_else(|_| SessionMetadata {
                id: id.clone(),
                created_at_ms: Utc::now().timestamp_millis(),
                updated_at_ms: Utc::now().timestamp_millis(),
                event_count: 0,
            })
        } else {
            SessionMetadata {
                id: id.clone(),
                created_at_ms: Utc::now().timestamp_millis(),
                updated_at_ms: Utc::now().timestamp_millis(),
                event_count: 0,
            }
        };

        meta.updated_at_ms = Utc::now().timestamp_millis();
        meta.event_count += records.len();
        tokio::fs::write(meta_path, serde_json::to_string_pretty(&meta)?).await?;

        Ok(())
    }

    async fn load_events(&self, id: &SessionId) -> Result<Vec<SessionEventRecord>> {
        let session_path = self.session_file_path(id);
        if !session_path.exists() {
            return Err(StorageError::SessionNotFound(id.to_string()));
        }

        let file = File::open(session_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut raw_records = Vec::new();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let record: SessionEventRecord = serde_json::from_str(trimmed)?;
            raw_records.push(record);
        }

        // 经过崩溃自愈处理后交付
        Ok(repair_session_records(raw_records))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut dir = tokio::fs::read_dir(&self.base_dir).await?;
        let mut result = Vec::new();

        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && path.file_name().and_then(|s| s.to_str()).map(|s| s.ends_with(".meta.json")).unwrap_or(false)
            {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok(meta) = serde_json::from_str::<SessionMetadata>(&content) {
                        result.push(meta);
                    }
                }
            }
        }

        result.sort_by_key(|m| m.created_at_ms);
        Ok(result)
    }

    async fn delete_session(&self, id: &SessionId) -> Result<()> {
        let _ = tokio::fs::remove_file(self.session_file_path(id)).await;
        let _ = tokio::fs::remove_file(self.meta_file_path(id)).await;
        Ok(())
    }
}

