//! Typed Saucepan core for the fresh implementation.
//!
//! Only models and the core facade form the supported library boundary. Source
//! providers, authority capabilities, and store mutation are implementation details.
//!
//! Downstream consumers cannot instantiate internal mutation paths:
//! ```compile_fail
//! use saucepan::core::store::Session;
//! ```
//! ```compile_fail
//! use saucepan::core::sources;
//! ```
pub mod core;

mod utils;

pub use core::models::{Error, ErrorKind, Recipe, Result};
