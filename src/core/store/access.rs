//! Execution leases are persisted as markers whose open native handles outlive
//! launcher failure. No PID or elapsed-time heuristic grants reclaim permission.
use crate::error::{Error, ErrorCode};
use crate::utils::lifetime::marker_held;
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
    fs::{self, File},
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
    pub(crate) fn project(&mut self, output: &OutputEntry, relative: Option<&str>) -> Result<()> {
        self.resource.directory = self.directory_view(output, relative)?;
        Ok(())
    }
    pub(crate) fn directory_view(
        &self,
        output: &OutputEntry,
        relative: Option<&str>,
    ) -> Result<PathBuf> {
        anyhow::ensure!(
            self.entry_uuid() == output.uuid,
            Error::new(ErrorCode::IntegrityError, "view output identity mismatch")
        );
        if let Some(relative) = relative {
            anyhow::ensure!(
                output
                    .files
                    .get(relative)
                    .is_some_and(|entry| entry.digest.is_none()),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "view is not a directory in the output manifest"
                )
            );
            let directory = beneath(self.directory(), relative)?;
            anyhow::ensure!(
                plain_metadata(&directory)?.is_dir(),
                Error::new(
                    ErrorCode::ContentUnavailable,
                    "view directory is unavailable"
                )
            );
            return Ok(directory);
        }
        Ok(self.directory().to_owned())
    }

    /// Used only by dedicated single-launch CLI/shim processes. Other spawns
    /// could inherit the same handle and conservatively extend its lifetime.
    pub(crate) fn inherit(&mut self) -> io::Result<()> {
        // The shared coordination guard excludes reclaim throughout creation
        // and inheritance, including the interval before the native marker locks.
        let file = crate::utils::lifetime::inheritable_marker(&self.marker_path)?;
        self.file = Some(file);
        Ok(())
    }
}

impl Store {
    pub(crate) fn resolve_planned(
        &self,
        entry: Uuid,
        mode: VerificationMode,
    ) -> Result<ResolvedResource> {
        if mode == VerificationMode::Metadata {
            let _lock = self.lock()?;
            self.authorize_output(entry, crate::models::claim::policy::Action::Lease)?;
            let output = self.output(entry)?;
            return Ok(ResolvedResource {
                entry_uuid: entry,
                directory: self.metadata_path(&output)?,
                verification: mode,
            });
        }
        Ok(self.lease_planned(entry, mode)?.resource().clone())
    }
    pub fn verify(&self, entry: Uuid) -> Result<ResolvedResource> {
        self.resolve(entry, VerificationMode::Full)
    }
    pub(crate) fn lease_planned(
        &self,
        entry: Uuid,
        mode: VerificationMode,
    ) -> Result<ResourceLease> {
        let (output, mut lease) = {
            let _lock = self.lock()?;
            self.authorize_output(entry, crate::models::claim::policy::Action::Lease)?;
            let output = self.output(entry)?;
            let lease = self.metadata_lease_locked(&output)?;
            (output, lease)
        };
        if mode == VerificationMode::Full {
            lease.verify(self, &output)?;
        }
        Ok(lease)
    }
    pub(crate) fn lease_claim_planned(
        &self,
        app: Uuid,
        claim: Uuid,
        mode: VerificationMode,
    ) -> Result<ResourceLease> {
        use anyhow::Context;
        let (output, mut lease) = {
            let _lock = self.lock()?;
            self.application(app)?;
            let application = self.registry(app)?;
            self.check_session(&application)?;
            let claim = application
                .claims
                .get(&claim)
                .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
            self.authorize_claim(
                &application,
                claim,
                crate::models::claim::policy::Action::Lease,
            )?;
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
        self.metadata_lease_with_timeout(output, Duration::from_secs(30))
    }
    pub(crate) fn metadata_lease_with_timeout(
        &self,
        output: &OutputEntry,
        timeout: Duration,
    ) -> Result<ResourceLease> {
        let entry = output.uuid;
        let directory = self.metadata_path(output)?;
        let coordination = LockGuard::acquire(
            &self.root.join(format!(".locks/use-{entry}")),
            LockMode::Shared,
            timeout,
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
