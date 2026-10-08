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

pub struct ResourceLease {
    entry: Uuid,
    directory: std::path::PathBuf,
    file: File,
    _coordination: LockGuard,
}
impl ResourceLease {
    pub fn entry_uuid(&self) -> Uuid {
        self.entry
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }

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
    pub fn lease(&self, entry: Uuid) -> Result<ResourceLease> {
        let _lock = self.lock()?;
        self.lease_locked(entry)
    }
    pub fn lease_claim(&self, app: Uuid, claim: Uuid) -> Result<ResourceLease> {
        use anyhow::Context;
        let _lock = self.lock()?;
        let application = self.application(app)?;
        let claim = application.claims.get(&claim).context("claim missing")?;
        self.lease_locked(claim.entry_uuid)
    }
    pub fn execution_lease(&self, entry: Uuid) -> Result<ResourceLease> {
        let _lock = self.lock()?;
        self.lease_locked(entry)
    }
    pub(crate) fn lease_locked(&self, entry: Uuid) -> Result<ResourceLease> {
        let directory = self.verified_path(&self.output(entry)?)?;
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
        Ok(ResourceLease {
            entry,
            directory,
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
