//! Execution leases are persisted as markers whose open native handles outlive
//! launcher failure. No PID or elapsed-time heuristic grants reclaim permission.
use crate::{
    core::Store,
    models::store::{OutputEntry, ResolvedResource, VerificationMode},
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
#[cfg(test)]
mod tests;

pub struct ResourceLease {
    resource: ResolvedResource,
    file: File,
    _coordination: LockGuard,
}
impl ResourceLease {
    pub fn entry_uuid(&self) -> Uuid {
        self.resource.entry_uuid
    }
    pub fn directory(&self) -> &Path {
        &self.resource.directory
    }
    pub fn resource(&self) -> &ResolvedResource {
        &self.resource
    }

    pub(crate) fn verify(&mut self, store: &Store, output: &OutputEntry) -> Result<()> {
        anyhow::ensure!(output.uuid == self.entry_uuid(), "lease identity mismatch");
        store.check_manifest(self.directory(), output)?;
        self.resource.verification = VerificationMode::Full;
        Ok(())
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
    pub fn resolve(&self, entry: Uuid, mode: VerificationMode) -> Result<ResolvedResource> {
        Ok(self.lease(entry, mode)?.resource().clone())
    }
    pub fn verify(&self, entry: Uuid) -> Result<ResolvedResource> {
        self.resolve(entry, VerificationMode::Full)
    }
    pub fn lease(&self, entry: Uuid, mode: VerificationMode) -> Result<ResourceLease> {
        let (output, mut lease) = {
            let _lock = self.lock()?;
            let output = self.output(entry)?;
            let lease = self.metadata_lease_locked(&output)?;
            (output, lease)
        };
        if mode == VerificationMode::Full {
            lease.verify(self, &output)?;
        }
        Ok(lease)
    }
    pub fn lease_claim(
        &self,
        app: Uuid,
        claim: Uuid,
        mode: VerificationMode,
    ) -> Result<ResourceLease> {
        use anyhow::Context;
        let (output, mut lease) = {
            let _lock = self.lock()?;
            let application = self.application(app)?;
            let claim = application.claims.get(&claim).context("claim missing")?;
            let output = self.output(claim.entry_uuid)?;
            let lease = self.metadata_lease_locked(&output)?;
            (output, lease)
        };
        if mode == VerificationMode::Full {
            lease.verify(self, &output)?;
        }
        Ok(lease)
    }
    pub fn execution_lease(&self, entry: Uuid) -> Result<ResourceLease> {
        self.lease(entry, VerificationMode::Full)
    }
    pub(crate) fn lease_locked(&self, entry: Uuid) -> Result<ResourceLease> {
        let output = self.output(entry)?;
        let mut lease = self.metadata_lease_locked(&output)?;
        lease.verify(self, &output)?;
        Ok(lease)
    }
    /// Caller holds mutation coordination until selection and native ownership complete.
    pub(crate) fn metadata_lease_locked(&self, output: &OutputEntry) -> Result<ResourceLease> {
        let entry = output.uuid;
        let directory = self.metadata_path(output)?;
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
            resource: ResolvedResource {
                entry_uuid: entry,
                directory,
                verification: VerificationMode::Metadata,
            },
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
