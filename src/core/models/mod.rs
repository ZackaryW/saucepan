//! Passive versioned inputs and results. No filesystem, process, or network I/O.
mod error;
mod recipe;
mod state;

pub use error::{Error, ErrorKind, Result};
pub use recipe::*;
pub use state::*;

pub const SCHEMA_VERSION: u32 = 1;
pub const EXPORT_VERSION: u32 = 1;
pub const PROTOCOL_VERSION: u32 = 1;
