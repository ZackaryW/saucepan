use crate::error::{Error, ErrorCode};
use crate::{
    core::{Store, index::Journal, policy},
    models::{
        index::{Claim, Record},
        policy::Identical,
        store::{Acquired, Availability, FileEntry, OutputEntry, Request, VerificationMode},
    },
    utils::{
        path::{beneath, plain_metadata},
        tree::{self, TreeLimits},
    },
};
use anyhow::{Result, bail, ensure};
use std::{collections::BTreeMap, path::PathBuf};
use uuid::Uuid;
mod reclaim;

pub(crate) struct Acquisition {
    pub acquired: Acquired,
    pub claim: Option<Claim>,
    lease: Option<ResourceLease>,
}

pub(crate) fn json_digest(value: &impl serde::Serialize) -> Result<String> {
    Ok(tree::digest(serde_json::to_vec(value)?.as_slice())?)
}
pub(crate) fn manifest(path: &std::path::Path) -> Result<BTreeMap<String, FileEntry>> {
    Ok(tree::inspect(path, TreeLimits::default())?
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                FileEntry {
                    digest: v.digest,
                    executable: v.executable,
                },
            )
        })
        .collect())
}
impl Store {
    pub fn output(&self, id: Uuid) -> Result<OutputEntry> {
        match self.record(id).map_err(|e| {
            super::index::classify_missing(e, ErrorCode::UnknownOutput, "output missing")
        })? {
            Record::Output(entry) => Ok(*entry),
            _ => bail!(Error::new(ErrorCode::UnknownOutput, "not an output entry")),
        }
    }
    pub fn path(&self, id: Uuid) -> Result<PathBuf> {
        Ok(self.resolve(id, VerificationMode::Metadata)?.directory)
    }
    pub(crate) fn metadata_path(&self, entry: &OutputEntry) -> Result<PathBuf> {
        ensure!(
            entry.state == Availability::Ready,
            Error::new(ErrorCode::ContentUnavailable, "entry is unavailable")
        );
        let path = beneath(&self.root, &format!("store/{}", entry.uuid))?;
        ensure!(
            plain_metadata(&path)?.is_dir(),
            "output root is not a directory"
        );
        Ok(path)
    }
    /// Full integrity boundary. Caller must retain mutation coordination or an entry lease.
    pub(crate) fn verified_path(&self, entry: &OutputEntry) -> Result<PathBuf> {
        let path = self.metadata_path(entry)?;
        self.check_manifest(&path, entry)?;
        Ok(path)
    }
    pub(crate) fn check_manifest(&self, path: &std::path::Path, entry: &OutputEntry) -> Result<()> {
        ensure!(
            self.access_manifest(path)? == entry.files,
            Error::new(
                ErrorCode::IntegrityError,
                "stored output integrity mismatch"
            )
        );
        Ok(())
    }
    fn access_manifest(&self, path: &std::path::Path) -> Result<BTreeMap<String, FileEntry>> {
        #[cfg(test)]
        if let Some(observer) = &self.access_observer {
            return Ok(
                tree::inspect_observed(path, TreeLimits::default(), &mut |event| observer(event))?
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            FileEntry {
                                digest: v.digest,
                                executable: v.executable,
                            },
                        )
                    })
                    .collect(),
            );
        }
        manifest(path)
    }
    pub fn acquire(&self, request: &Request) -> Result<Acquired> {
        self.acquire_with(request, &Default::default())
    }
    pub fn acquire_with(
        &self,
        request: &Request,
        options: &super::source::AcquireOptions,
    ) -> Result<Acquired> {
        Ok(self.acquire_inner(request, options, None)?.acquired)
    }
    pub(crate) fn acquire_inner(
        &self,
        request: &Request,
        options: &super::source::AcquireOptions,
        claim: Option<super::index::claims::ClaimTarget>,
    ) -> Result<Acquisition> {
        self.acquire_protected(request, options, claim, false)
    }
    pub(crate) fn acquire_candidate(
        &self,
        request: &Request,
        options: &super::source::AcquireOptions,
    ) -> Result<(Acquired, ResourceLease)> {
        let result = self.acquire_protected(request, options, None, true)?;
        Ok((
            result.acquired,
            result.lease.expect("requested candidate lease"),
        ))
    }
    fn acquire_protected(
        &self,
        request: &Request,
        options: &super::source::AcquireOptions,
        claim: Option<super::index::claims::ClaimTarget>,
        retain: bool,
    ) -> Result<Acquisition> {
        policy::validate(request).map_err(|e| {
            e.context(Error::new(
                ErrorCode::InvalidRequest,
                "invalid acquisition request",
            ))
        })?;
        let prepared = self.prepare(request, options).map_err(|e| {
            if e.downcast_ref::<Error>().is_some() {
                e
            } else {
                e.context(Error::new(
                    ErrorCode::SourceError,
                    "source acquisition failed",
                ))
            }
        })?;
        let content = prepared.temp.path().join("content");
        let files = manifest(&content)?;
        for bin in &request.selection.bin {
            ensure!(
                files.get(bin).is_some_and(|f| f.digest.is_some()),
                "binary does not exist: {bin}"
            );
        }
        let content_digest = json_digest(&files)?;
        let item_key = json_digest(&(request.source_uuid, &request.selector, &request.selection))?;
        let fingerprint = json_digest(&(
            &item_key,
            &prepared.resolved,
            &content_digest,
            &prepared.dependencies,
            &request.policies.eviction,
            &request.policies.protection,
        ))?;
        #[cfg(test)]
        if claim.is_some()
            && let Some(observer) = &self.before_claim_commit
        {
            observer();
        }
        let _lock = self.lock()?;
        let records = self.records()?;
        let outputs: Vec<_> = records
            .iter()
            .filter_map(|r| {
                if let Record::Output(e) = r {
                    Some(e.as_ref())
                } else {
                    None
                }
            })
            .collect();
        if request.policies.identical == Identical::Reuse
            && let Some(entry) = outputs
                .iter()
                .find(|e| e.fingerprint == fingerprint && e.state == Availability::Ready)
        {
            let directory = self.verified_path(entry)?;
            let mut committed_claim = None;
            if let Some(target) = &claim {
                let record = self.claim_record(target, request, entry.uuid)?;
                if let Record::Application(app) = &record {
                    committed_claim = Some(app.claims[&target.id].clone());
                }
                self.commit(Journal {
                    records: vec![record],
                    stage: None,
                    ..Default::default()
                })?;
            }
            let lease = retain
                .then(|| self.metadata_lease_locked(entry))
                .transpose()?;
            return Ok(Acquisition {
                acquired: Acquired {
                    entry: (**entry).clone(),
                    directory,
                    reused: true,
                    update_checked: prepared.update_checked,
                    stale: prepared.stale,
                    reclaim: self.reclaim_locked()?,
                },
                lease,
                claim: committed_claim,
            });
        }
        let sequence = outputs
            .iter()
            .map(|e| e.sequence)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("sequence overflow"))?;
        let entry = OutputEntry {
            uuid: Uuid::new_v4(),
            request: request.clone(),
            item_key,
            fingerprint,
            resolved: prepared.resolved,
            content_digest,
            files,
            dependencies: prepared.dependencies,
            sequence,
            state: Availability::Ready,
        };
        let stage_name = prepared
            .temp
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let mut records = vec![Record::Output(Box::new(entry.clone()))];
        let mut committed_claim = None;
        if let Some(target) = &claim {
            let record = self.claim_record(target, request, entry.uuid)?;
            if let Record::Application(app) = &record {
                committed_claim = Some(app.claims[&target.id].clone());
            }
            records.push(record);
        }
        self.commit(Journal {
            records,
            stage: Some((stage_name, entry.uuid)),
            ..Default::default()
        })?;
        let directory = self.root.join("store").join(entry.uuid.to_string());
        let lease = retain
            .then(|| self.metadata_lease_locked(&entry))
            .transpose()?;
        Ok(Acquisition {
            acquired: Acquired {
                entry,
                directory,
                reused: false,
                update_checked: prepared.update_checked,
                stale: prepared.stale,
                reclaim: self.reclaim_locked()?,
            },
            lease,
            claim: committed_claim,
        })
    }
}

pub mod access;
pub use access::ResourceLease;
