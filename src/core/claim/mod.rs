//! Claim registry operations and application permission decisions.
pub(crate) mod composition;
mod ensure;
mod matching;
pub(crate) mod operations;
pub mod persistence;
pub mod policy;
mod recovery;
pub mod registry;
pub mod security;
pub mod session;
