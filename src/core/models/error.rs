use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable errors at the native/CLI boundary. Hidden objects use NotFound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum ErrorKind {
    NotFound = 1,
    Source = 2,
    Config = 3,
    Conflict = 4,
    Internal = 5,
    Authority = 6,
    Integrity = 7,
    Busy = 8,
    Compatibility = 9,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    // Kept private so diagnostics go through a deliberate constructor.
    message: String,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn exit_code(&self) -> i32 {
        self.kind as i32
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn not_found() -> Self {
        Self::new(ErrorKind::NotFound, "resource not found")
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
