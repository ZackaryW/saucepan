use crate::{
    core::{Store, index::Journal, policy},
    models::{
        index::Record,
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
        match self.record(id)? {
            Record::Output(entry) => Ok(*entry),
            _ => bail!("not an output entry"),
        }
    }
    pub fn path(&self, id: Uuid) -> Result<PathBuf> {
        Ok(self.resolve(id, VerificationMode::Metadata)?.directory)
    }
    pub(crate) fn metadata_path(&self, entry: &OutputEntry) -> Result<PathBuf> {
        ensure!(
            entry.state == Availability::Ready,
            "entry is unavailable: {:?}",
            entry.state
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
            "stored output integrity mismatch"
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
        self.acquire_inner(request, options, None)
    }
    pub(crate) fn acquire_inner(
        &self,
        request: &Request,
        options: &super::source::AcquireOptions,
        claim: Option<super::index::claims::ClaimTarget>,
    ) -> Result<Acquired> {
        policy::validate(request)?;
        let prepared = self.prepare(request, options)?;
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
            if let Some(target) = &claim {
                self.commit(Journal {
                    records: vec![self.claim_record(target, request, entry.uuid)?],
                    stage: None,
                    ..Default::default()
                })?;
            }
            return Ok(Acquired {
                entry: (**entry).clone(),
                directory,
                reused: true,
                update_checked: prepared.update_checked,
                stale: prepared.stale,
                reclaim: self.reclaim_locked()?,
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
        if let Some(target) = &claim {
            records.push(self.claim_record(target, request, entry.uuid)?);
        }
        self.commit(Journal {
            records,
            stage: Some((stage_name, entry.uuid)),
            ..Default::default()
        })?;
        let directory = self.root.join("store").join(entry.uuid.to_string());
        Ok(Acquired {
            entry,
            directory,
            reused: false,
            update_checked: prepared.update_checked,
            stale: prepared.stale,
            reclaim: self.reclaim_locked()?,
        })
    }
}

pub mod access;
pub use access::ResourceLease;
