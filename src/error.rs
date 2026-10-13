//! Stable error identities. Messages are explanatory, not a machine protocol.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    AuthenticationRequired,
    AuthenticationFailed,
    KeyUnavailable,
    IntegrityError,
    UnknownApplication,
    UnknownClaim,
    UnknownOutput,
    ContentUnavailable,
    RecoveryRequired,
    ExactContentUnavailable,
    RevisionConflict,
    ClaimContextConflict,
    ClaimOverlap,
    PolicyConflict,
    AccessDenied,
    SourceError,
    IoError,
    UnsupportedStore,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    pub details: BTreeMap<String, String>,
}
impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: BTreeMap::new(),
        }
    }
    /// Retains typed causes without interpreting their Display strings.
    pub fn from_anyhow(error: &anyhow::Error) -> Self {
        if let Some(typed) = error.downcast_ref::<Self>() {
            return typed.clone();
        }
        if error.downcast_ref::<serde_json::Error>().is_some() {
            return Self::new(ErrorCode::InvalidRequest, "invalid JSON input");
        }
        if error.downcast_ref::<std::io::Error>().is_some() {
            return Self::new(ErrorCode::IoError, "I/O operation failed");
        }
        Self::new(ErrorCode::InternalError, "operation failed")
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::new(ErrorCode::IoError, "I/O operation failed")
    }
}
pub type Result<T> = std::result::Result<T, Error>;
