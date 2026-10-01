use crate::error::{Result, SessionError};
use crate::event::{SessionEvent, SessionEventRecord};
use chrono::Utc;
use dsh_types::{SessionId, SessionSeq};

/// An in-memory event-sourced session holding an append-only log of events.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    id: SessionId,
    created_at_ms: i64,
    records: Vec<SessionEventRecord>,
}

impl Session {
    /// Create a new empty session with a given ID.
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            created_at_ms: Utc::now().timestamp_millis(),
            records: Vec::new(),
        }
    }

    /// Reconstruct a session from existing event records with continuity validation.
    pub fn from_records(id: SessionId, records: Vec<SessionEventRecord>) -> Result<Self> {
        let session = Self {
            id,
            created_at_ms: records
                .first()
                .map(|r| r.timestamp_ms)
                .unwrap_or_else(|| Utc::now().timestamp_millis()),
            records,
        };
        session.validate_continuity()?;
        Ok(session)
    }

    /// Return the session identifier.
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// Return the session creation timestamp in epoch milliseconds.
    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    /// Append an event to the log and return the assigned sequence number.
    pub fn append(&mut self, event: SessionEvent) -> SessionSeq {
        let next_seq = match self.records.last() {
            Some(last) => last.seq.next(),
            None => SessionSeq::ZERO,
        };
        self.records.push(SessionEventRecord::new(next_seq, event));
        next_seq
    }

    /// Borrow the slice of all committed event records.
    pub fn records(&self) -> &[SessionEventRecord] {
        &self.records
    }

    /// Total count of events.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the log is currently empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Return the highest sequence number committed so far, or None if empty.
    pub fn last_seq(&self) -> Option<SessionSeq> {
        self.records.last().map(|r| r.seq)
    }

    /// Validate that all sequence numbers are strictly contiguous starting at ZERO.
    pub fn validate_continuity(&self) -> Result<()> {
        for (idx, record) in self.records.iter().enumerate() {
            let expected = SessionSeq::new(idx as u64);
            if record.seq != expected {
                return Err(SessionError::SequenceMismatch {
                    expected,
                    actual: record.seq,
                });
            }
        }
        Ok(())
    }
}

