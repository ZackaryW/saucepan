//! Dedicated managed process boundary. Resource lifetime ownership lives in store access.
pub use crate::core::store::access::ResourceLease as ExecutionLease;
use crate::{
    core::{Store, store::ResourceLease},
    utils::path::{beneath, plain_metadata},
};
use anyhow::Result;
use std::{fs, path::Path};
use uuid::Uuid;
impl Store {
    /// Launch committed binding with inherited streams and literal arguments.
    pub fn launch(&self, app: Uuid, args: &[std::ffi::OsString], saucepan: bool) -> Result<i32> {
        let (command, lease) = self.prepare_launch(app, args, saucepan)?;
        launch_leased(command, lease)
    }
    pub(crate) fn prepare_launch(
        &self,
        app: Uuid,
        args: &[std::ffi::OsString],
        saucepan: bool,
    ) -> Result<(std::process::Command, ResourceLease)> {
        use anyhow::{Context, ensure};
        let (path, lease) = {
            let _lock = self.lock()?;
            let binding = self
                .application(app)?
                .active
                .context("application has no active binding")?;
            let output = self.output(binding.entry_uuid)?;
            ensure!(
                output.request.selection.bin.contains(&binding.executable),
                "executable was not declared"
            );
            let lease = self.metadata_lease_locked(&output)?;
            let path = beneath(lease.directory(), &binding.executable)?;
            ensure!(
                plain_metadata(&path)?.is_file(),
                "target is not a regular file"
            );
            ensure!(path != std::env::current_exe()?, "recursive shim target");
            (path, lease)
        };
        let mut command = std::process::Command::new(path);
        if saucepan {
            for (i, arg) in args.iter().enumerate() {
                if arg == "--root" {
                    ensure!(
                        Path::new(args.get(i + 1).context("missing root")?).canonicalize()?
                            == self.root,
                        "shim root mismatch"
                    );
                }
                if let Some(value) = arg.to_str().and_then(|s| s.strip_prefix("--root=")) {
                    ensure!(
                        Path::new(value).canonicalize()? == self.root,
                        "shim root mismatch"
                    );
                }
            }
            if !args
                .iter()
                .any(|a| a == "--root" || a.to_str().is_some_and(|s| s.starts_with("--root=")))
            {
                command.arg("--root").arg(&self.root);
            }
        }
        command.args(args);
        Ok((command, lease))
    }
}

/// Only the dedicated CLI/shim processes call this boundary. Never use it for
/// in-process resource access: Unix replaces the caller's process with the tool.
pub(crate) fn launch_leased(
    mut command: std::process::Command,
    mut lease: ResourceLease,
) -> Result<i32> {
    lease.inherit()?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().into())
    }
    #[cfg(windows)]
    {
        // Remain in the console group. Inherited native ownership survives
        // wrapper termination until the target closes its handle or exits.
        let status = command.spawn()?.wait()?;
        drop(lease);
        Ok(status.code().unwrap_or(1))
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShimConfig {
    pub app: Uuid,
    pub saucepan: bool,
}

pub fn shim_main() -> Result<i32> {
    use anyhow::Context;
    let exe = std::env::current_exe()?;
    let root = exe
        .parent()
        .and_then(Path::parent)
        .context("shim must live in root/bin")?;
    let _shim = super::shims::shim_lock(
        root,
        exe.file_name()
            .context("shim filename missing")?
            .to_str()
            .context("non-UTF8 shim name")?,
        crate::utils::lock::LockMode::Shared,
    )?;
    let config: ShimConfig = serde_json::from_slice(&fs::read(exe.with_extension("shim.json"))?)?;
    let store = Store::open(root)?;
    store.launch(
        config.app,
        &std::env::args_os().skip(1).collect::<Vec<_>>(),
        config.saucepan,
    )
}
