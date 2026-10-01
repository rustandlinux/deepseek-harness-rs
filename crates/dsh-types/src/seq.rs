use serde::{Deserialize, Serialize};
use std::fmt;

/// Monotonically increasing event sequence number in an event-sourced session log.
/// Represents a non-negative continuous index (0, 1, 2, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionSeq(u64);

impl SessionSeq {
    /// Zero sequence watermark (the first event in a session).
    pub const ZERO: Self = Self(0);

    /// Create from a raw u64.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw integer value.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Produce the next sequential number.
    pub const fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl fmt::Display for SessionSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for SessionSeq {
    fn from(val: u64) -> Self {
        Self(val)
    }
}

impl From<SessionSeq> for u64 {
    fn from(seq: SessionSeq) -> Self {
        seq.0
    }
}

/// Offset into the session log (prefix length or gap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionLogOffset(u64);

impl SessionLogOffset {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for SessionLogOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for SessionLogOffset {
    fn from(val: u64) -> Self {
        Self(val)
    }
}

