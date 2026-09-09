//! Shared acquisition and encrypted app-index behavior; adapters only translate calls.
pub mod models;
pub mod policies;
mod store;
pub use store::Store;
mod acquisition;
mod content;
pub mod platform;
pub mod sources;
