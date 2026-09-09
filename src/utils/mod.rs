//! Domain-neutral helpers. App, source, index, and policy rules belong in the core.
//!
//! - [`hash`] and [`json`]: streaming SHA-256 and sorted serialization.
//! - [`crypto`]: random bytes, HKDF, HMAC, and authenticated byte encryption.
//! - [`path`]: logical paths and host-specific filename validation.
//! - [`lock`] and [`fs`]: bounded writer locking and staged file/directory publication.
//! - [`tree`] and [`archive`]: filtered tree copies and ZIP stream round trips.
//!
//! Filters, byte/entry limits, key material, and authentication contexts are supplied
//! by callers. Helpers do not assign source IDs, choose retention rules, access a
//! keyring, store app settings, or define index/token formats. Use library primitives
//! directly where an extra wrapper adds no behavior.
//!
//! Directory publication requires a trusted parent and cooperating writers holding
//! the same lock. Inputs must remain stable during export/copy. These operations
//! preserve an occupied destination and clean failed staging, but do not implement
//! multi-file transactions or protection from a hostile process running as the user.
//!
//! ```
//! use saucepan::utils::{archive, fs};
//! # fn example(source: &std::path::Path, output: &std::path::Path) -> std::io::Result<()> {
//! fs::atomic_write(output, |file| {
//!     archive::write_zip(source, file, |path, _| {
//!         !path.split('/').any(|component| component == ".git")
//!     })?;
//!     Ok(())
//! })
//! # }
//! ```

pub mod archive;
pub mod crypto;
pub mod fs;
pub mod hash;
pub mod json;
pub mod lock;
pub mod path;
pub mod tree;

#[cfg(test)]
mod tests;
