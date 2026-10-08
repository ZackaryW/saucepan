//! Execution leases are persisted as markers whose open native handles outlive
//! launcher failure. No PID or elapsed-time heuristic grants reclaim permission.
use crate::{
    core::Store,
    models::store::{OutputEntry, ResolvedResource, VerificationMode},
    utils::{
        lock::{LockGuard, LockMode},
        path::{beneath, plain_metadata},
    },
};
use anyhow::Result;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
#[cfg(test)]
mod tests;

pub struct ResourceLease {
    resource: ResolvedResource,
    marker_path: PathBuf,
    file: Option<File>,
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

    /// Used only by dedicated single-launch CLI/shim processes. Other spawns
    /// could inherit the same handle and conservatively extend its lifetime.
    pub(crate) fn inherit(&mut self) -> io::Result<()> {
        // The shared coordination guard excludes reclaim throughout creation
        // and inheritance, including the interval before the native marker locks.
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(3);
        }
        let file = options.open(&self.marker_path)?;
        #[cfg(unix)]
        file.lock_shared()?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation};
            // SAFETY: the borrowed file handle remains valid for this call.
            unsafe {
                SetHandleInformation(
                    HANDLE(file.as_raw_handle()),
                    HANDLE_FLAG_INHERIT.0,
                    HANDLE_FLAG_INHERIT,
                )
            }
            .map_err(io::Error::other)?;
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let fd = file.as_raw_fd();
            // SAFETY: fcntl operates on this guard's valid file descriptor.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        self.file = Some(file);
        Ok(())
    }
}

impl Store {
    pub fn resolve(&self, entry: Uuid, mode: VerificationMode) -> Result<ResolvedResource> {
        if mode == VerificationMode::Metadata {
            let _lock = self.lock()?;
            let output = self.output(entry)?;
            return Ok(ResolvedResource {
                entry_uuid: entry,
                directory: self.metadata_path(&output)?,
                verification: mode,
            });
        }
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
    /// Caller holds mutation coordination until selection and native ownership complete.
    pub(crate) fn metadata_lease_locked(&self, output: &OutputEntry) -> Result<ResourceLease> {
        let entry = output.uuid;
        let directory = self.metadata_path(output)?;
        let coordination = LockGuard::acquire(
            &self.root.join(format!(".locks/use-{entry}")),
            LockMode::Shared,
            Duration::from_secs(30),
        )?;
        let marker_path = self
            .root
            .join(format!(".locks/live-{entry}-{}", Uuid::new_v4()));
        Ok(ResourceLease {
            resource: ResolvedResource {
                entry_uuid: entry,
                directory,
                verification: VerificationMode::Metadata,
            },
            marker_path,
            file: None,
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

    /// Caller holds mutation coordination. Entry coordination excludes a launch
    /// between marker creation and native locking, including older launchers.
    pub(crate) fn sweep_closed_markers_locked(&self) -> Result<()> {
        for candidate in fs::read_dir(self.root.join(".locks"))? {
            let name = candidate?.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(entry) = marker_entry(name) else {
                continue;
            };
            let _entry_lock = match LockGuard::acquire(
                &self.root.join(format!(".locks/use-{entry}")),
                LockMode::Exclusive,
                Duration::ZERO,
            ) {
                Ok(lock) => lock,
                Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                Err(e) => return Err(e.into()),
            };
            let path = beneath(&self.root, &format!(".locks/{name}"))?;
            if plain_metadata(&path)?.is_file() && !marker_held(&path)? {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

fn marker_entry(name: &str) -> Option<Uuid> {
    let parts = name.strip_prefix("live-")?;
    let entry = Uuid::parse_str(parts.get(..36)?).ok()?;
    let marker = Uuid::parse_str(parts.get(37..)?).ok()?;
    (name == format!("live-{entry}-{marker}")).then_some(entry)
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
