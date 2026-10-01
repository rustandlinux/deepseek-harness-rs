use dsh_types::SessionSeq;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Non-contiguous sequence detected: expected {expected}, got {actual}")]
    SequenceMismatch {
        expected: SessionSeq,
        actual: SessionSeq,
    },

    #[error("Corrupted event log: {0}")]
    CorruptLog(String),

    #[error("Turn mismatch: cannot close turn {turn_to_close} while in turn {current_turn}")]
    TurnMismatch {
        current_turn: u32,
        turn_to_close: u32,
    },
}

pub type Result<T> = std::result::Result<T, SessionError>;

