use crate::{
    core::{Store, policy::catalog::PolicyCatalog},
    error::{Error, ErrorCode},
    models::{lease::GroupRequest, policy::PolicyRecord},
};
use anyhow::{Result, ensure};
use serde::de::DeserializeOwned;
use std::{ffi::OsString, fs::File, io::Read, path::Path, process::Command};

pub(super) fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(GroupRequest::MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= GroupRequest::MAX_JSON_BYTES,
        Error::new(ErrorCode::InvalidRequest, "group input exceeds 1 MiB")
    );
    Ok(serde_json::from_slice(&bytes)?)
}
pub(super) fn run(
    store: &Store,
    file: &Path,
    policies: Option<&Path>,
    command: &[OsString],
) -> Result<i32> {
    let request: GroupRequest = read(file)?;
    let catalog = match policies {
        Some(path) => PolicyCatalog::new(read::<Vec<PolicyRecord>>(path)?)?,
        None => PolicyCatalog::default(),
    };
    let lease = store.lease_group(&request, &catalog)?;
    let mut process = Command::new(&command[0]);
    process.args(&command[1..]);
    crate::core::lease::launch::launch_group(process, lease)
}
