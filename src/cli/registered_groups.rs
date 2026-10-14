use crate::{
    core::{Store, group::PreparedGroup, source::AcquireOptions},
    models::pin::GroupPinRequest,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::{ffi::OsString, path::PathBuf, process::Command};
use uuid::Uuid;

#[derive(clap::Subcommand)]
pub(super) enum GroupCommand {
    /// Register persistent intent without retrieving or pinning resources.
    RegisterGroup {
        file: PathBuf,
    },
    ShowGroup {
        uuid: Uuid,
    },
    ListGroups {
        #[arg(long)]
        key: Option<String>,
    },
    ReplaceGroup {
        uuid: Uuid,
        file: PathBuf,
        #[arg(long)]
        expected_revision: u64,
    },
    RemoveGroup {
        uuid: Uuid,
        #[arg(long)]
        expected_revision: u64,
    },
    /// Prepare and atomically select a complete group. Retention after exit requires --pin.
    PrepareGroup {
        uuid: Uuid,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long, requires = "expected_revision")]
        pin: Option<PathBuf>,
    },
    /// Run the existing generation without retrieval or repair.
    RunRegisteredGroup {
        uuid: Uuid,
        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<OsString>,
    },
    /// Prepare, publish, and transfer continuous native guards to the consumer.
    PrepareRunGroup {
        uuid: Uuid,
        #[arg(long)]
        expected_revision: Option<u64>,
        #[arg(long, requires = "expected_revision")]
        pin: Option<PathBuf>,
        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<OsString>,
    },
    PinGroup {
        uuid: Uuid,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        pin: PathBuf,
    },
}
fn prepare(
    store: &Store,
    uuid: Uuid,
    revision: Option<u64>,
    pin: Option<PathBuf>,
    options: &AcquireOptions,
) -> Result<PreparedGroup> {
    match pin {
        Some(file) => store.prepare_group_with_pin(
            uuid,
            revision.ok_or_else(|| anyhow::anyhow!("pin requires expected revision"))?,
            options,
            &super::group::read::<GroupPinRequest>(&file)?,
        ),
        None => store.prepare_group(uuid, revision, options),
    }
}
fn run(prepared: PreparedGroup, args: Vec<OsString>) -> Result<Value> {
    let mut command = Command::new(&args[0]);
    command.args(&args[1..]);
    let code = crate::core::lease::launch::launch_group(command, prepared.lease)?;
    std::process::exit(code)
}
pub(super) fn execute(
    store: &Store,
    command: GroupCommand,
    options: &AcquireOptions,
) -> Result<Value> {
    Ok(match command {
        GroupCommand::RegisterGroup { file } => {
            serde_json::to_value(store.register_group(&super::group::read(&file)?)?)?
        }
        GroupCommand::ShowGroup { uuid } => serde_json::to_value(store.group(uuid)?)?,
        GroupCommand::ListGroups { key } => serde_json::to_value(store.groups(key.as_deref())?)?,
        GroupCommand::ReplaceGroup {
            uuid,
            file,
            expected_revision,
        } => serde_json::to_value(store.replace_group(
            uuid,
            expected_revision,
            &super::group::read(&file)?,
        )?)?,
        GroupCommand::RemoveGroup {
            uuid,
            expected_revision,
        } => {
            store.remove_group(uuid, expected_revision)?;
            json!({"removed":uuid})
        }
        GroupCommand::PrepareGroup {
            uuid,
            expected_revision,
            pin,
        } => {
            let prepared = prepare(store, uuid, expected_revision, pin, options)?;
            json!({"group":prepared.group,"pin":prepared.pin,"resources":prepared.lease.descriptor(),"stale":prepared.stale})
        }
        GroupCommand::RunRegisteredGroup { uuid, command } => {
            return run(store.open_group(uuid)?, command);
        }
        GroupCommand::PrepareRunGroup {
            uuid,
            expected_revision,
            pin,
            command,
        } => {
            return run(
                prepare(store, uuid, expected_revision, pin, options)?,
                command,
            );
        }
        GroupCommand::PinGroup {
            uuid,
            expected_revision,
            pin,
        } => serde_json::to_value(store.pin_group(
            uuid,
            expected_revision,
            &super::group::read(&pin)?,
        )?)?,
    })
}
