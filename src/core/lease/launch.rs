//! Dedicated process boundary only: native GroupLease never inherits ownership.
use super::GroupLease;
use crate::{
    core::{consumer, launch::launch_command},
    error::{Error, ErrorCode},
};
use anyhow::{Result, ensure};
use std::{fs::OpenOptions, io::Write, process::Command, rc::Rc};

pub(crate) fn launch_group(mut command: Command, mut group: GroupLease) -> Result<i32> {
    consumer::sweep_temporary_consumers()?;
    let temporary = consumer::temporary_workspace(None)?;
    let path = temporary.path().canonicalize()?.join("group.json");
    let bytes = serde_json::to_vec(group.descriptor())?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        Error::new(ErrorCode::InvalidRequest, "group descriptor exceeds 16 MiB")
    );
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?
        .write_all(&bytes)?;
    let _temporary = consumer::temporary_marker(&mut Some(Rc::new(temporary)))?;
    for lease in group.leases.values_mut() {
        lease.inherit()?;
    }
    command.env("SAUCEPAN_RESOURCE_GROUP_FILE", &path);
    for key in [
        "SAUCEPAN_RESOURCE_DIR",
        "SAUCEPAN_SOURCE_PATH",
        "SAUCEPAN_SOURCE_KIND",
        "SAUCEPAN_SOURCE_COMMIT",
    ] {
        command.env_remove(key);
    }
    let result = launch_command(command);
    drop(group);
    result
}
