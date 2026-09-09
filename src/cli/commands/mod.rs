//! Commands translate CLI inputs and typed core results. They must not
//! invoke source providers or mutate authority/store/snapshot records directly.
use saucepan::{Error, ErrorKind, core::models::Result};
use std::{io::Write, path::Path};

pub(super) fn validate_preferences(root: &Path) -> Result<()> {
    let bytes = super::read(
        &root.join("saucepan.toml"),
        1024 * 1024,
        "cannot read saucepan.toml",
    )?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new(ErrorKind::Config, "invalid saucepan.toml"))?;
    let _: toml::Value =
        toml::from_str(text).map_err(|_| Error::new(ErrorKind::Config, "invalid saucepan.toml"))?;
    Ok(())
}
pub(super) fn json(value: &impl serde::Serialize) -> Result<()> {
    let mut encoded = serde_json::to_vec(value)
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode command result"))?;
    encoded.push(b'\n');
    bytes(&encoded)
}
pub(super) fn ndjson(values: &[impl serde::Serialize]) -> Result<()> {
    let mut encoded = Vec::new();
    for value in values {
        serde_json::to_writer(&mut encoded, value)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode command result"))?;
        encoded.push(b'\n');
    }
    bytes(&encoded)
}
pub(super) fn bytes(bytes: &[u8]) -> Result<()> {
    std::io::stdout()
        .lock()
        .write_all(bytes)
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot write command result"))
}
