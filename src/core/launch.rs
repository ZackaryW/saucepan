//! Execution leases are persisted as markers whose open native handles outlive
//! launcher failure. No PID or elapsed-time heuristic grants reclaim permission.
use crate::{
    core::Store,
    utils::{
        lock::{LockGuard, LockMode},
        path::beneath,
    },
};
use anyhow::Result;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
    time::Duration,
};
use uuid::Uuid;

pub struct ExecutionLease {
    file: File,
    _coordination: LockGuard,
}
impl ExecutionLease {
    /// Used only by the dedicated single-launch shim. Other concurrent spawns
    /// could inherit the same handle and conservatively extend its lifetime.
    pub(crate) fn inherit(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation};
            // SAFETY: the borrowed file handle remains valid for this call.
            unsafe {
                SetHandleInformation(
                    HANDLE(self.file.as_raw_handle()),
                    HANDLE_FLAG_INHERIT.0,
                    HANDLE_FLAG_INHERIT,
                )
            }
            .map_err(io::Error::other)?;
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let fd = self.file.as_raw_fd();
            // SAFETY: fcntl operates on this guard's valid file descriptor.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        Ok(())
    }
}

impl Store {
    pub fn execution_lease(&self, entry: Uuid) -> Result<ExecutionLease> {
        let _lock = self.lock()?;
        self.lease_locked(entry)
    }
    pub(crate) fn lease_locked(&self, entry: Uuid) -> Result<ExecutionLease> {
        self.verified_path(&self.output(entry)?)?;
        let coordination = LockGuard::acquire(
            &self.root.join(format!(".locks/use-{entry}")),
            LockMode::Shared,
            Duration::from_secs(30),
        )?;
        let path = self
            .root
            .join(format!(".locks/live-{entry}-{}", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(3);
        }
        let file = options.open(path)?;
        #[cfg(unix)]
        file.lock_shared()?;
        Ok(ExecutionLease {
            file,
            _coordination: coordination,
        })
    }
    pub(crate) fn has_live_execution(&self, entry: Uuid) -> Result<bool> {
        let prefix = format!("live-{entry}-");
        for candidate in fs::read_dir(self.root.join(".locks"))? {
            let candidate = candidate?;
            let name = candidate.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&prefix) {
                continue;
            }
            let path = beneath(&self.root, &format!(".locks/{name}"))?;
            if marker_held(&path)? {
                return Ok(true);
            }
            fs::remove_file(path)?;
        }
        Ok(false)
    }
}
fn marker_held(path: &Path) -> io::Result<bool> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
        match options.open(path) {
            Ok(_file) => Ok(false),
            Err(e) if e.raw_os_error() == Some(32) => Ok(true),
            Err(e) => Err(e),
        }
    }
    #[cfg(unix)]
    {
        let file = options.open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(false),
            Err(std::fs::TryLockError::WouldBlock) => Ok(true),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }
}

impl Store {
    /// Launch committed binding with inherited streams and literal arguments.
    pub fn launch(&self, app: Uuid, args: &[std::ffi::OsString], saucepan: bool) -> Result<i32> {
        use anyhow::{Context, ensure};
        let (path, lease) = {
            let _lock = self.lock()?;
            let binding = self
                .application(app)?
                .active
                .context("application has no active binding")?;
            let root = self.verified_path(&self.output(binding.entry_uuid)?)?;
            let path = beneath(&root, &binding.executable)?;
            ensure!(path != std::env::current_exe()?, "recursive shim target");
            let lease = self.lease_locked(binding.entry_uuid)?;
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
        lease.inherit()?;
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // exec retains the marker's native shared lock and preserves signals.
            Err(command.exec().into())
        }
        #[cfg(windows)]
        {
            // Keep the console group so Ctrl-C reaches the target. The shim's
            // default handler may exit; the inherited marker still protects it.
            let status = command.spawn()?.wait()?;
            drop(lease);
            Ok(status.code().unwrap_or(1))
        }
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
    let config: ShimConfig = serde_json::from_slice(&fs::read(exe.with_extension("shim.json"))?)?;
    let store = Store::open(root)?;
    store.launch(
        config.app,
        &std::env::args_os().skip(1).collect::<Vec<_>>(),
        config.saucepan,
    )
}
