//! Source addresses, native read guards, configuration and explicit retirement.
use super::RetrievedResource;
use crate::{
    core::{
        Store,
        index::Journal,
        source::{git::normalize, retrieve},
    },
    error::{Error, ErrorCode},
    models::{
        index::Record,
        policy::Freshness,
        retrieval::{
            MaterialKind, Ownership, RetrievalRecord, RetrievalState, RetrieveRequest,
            RetrievedDescriptor, SourceInput,
        },
        source::{Resolved, Source, SourceRecord},
    },
    utils::{
        lock::{LockGuard, LockMode},
        path::beneath,
        source::git::{Git, RepositoryMaterial},
    },
};
use anyhow::{Result, bail, ensure};
use std::{fs, path::PathBuf, time::Duration};
use uuid::Uuid;
impl Store {
    pub fn retrieval_record(&self, id: Uuid) -> Result<RetrievalRecord> {
        match self.record(id)? {
            Record::Retrieval(record) => Ok(*record),
            _ => bail!(Error::new(
                ErrorCode::InvalidRequest,
                "not a retrieval record"
            )),
        }
    }

    /// An address only: no refresh and no retained native lifetime.
    pub fn source_path(&self, id: Uuid) -> Result<PathBuf> {
        let record = self.retrieval_record(id)?;
        ensure!(
            record.state == RetrievalState::Ready,
            Error::new(ErrorCode::ContentUnavailable, "retrieval was purged")
        );
        let path = self.retrieval_path(&record)?;
        crate::utils::path::plain_metadata(&path)?;
        Ok(path)
    }

    pub fn lease_retrieval(&self, id: Uuid) -> Result<RetrievedResource> {
        ensure!(
            self.session.is_none(),
            Error::new(ErrorCode::AccessDenied, "use scoped source access")
        );
        let source = self.retrieval_record(id)?.source_uuid;
        let guard = LockGuard::acquire(
            &self.root.join(format!(".locks/source-{source}")),
            LockMode::Shared,
            Duration::from_secs(150),
        )?;
        let record = self.retrieval_record(id)?;
        self.verify_retrieval(&record, &record.request)?;
        self.retrieved_handle(&record, &record.request, guard, true, false, false)
    }

    pub(crate) fn retrieval_path(&self, record: &RetrievalRecord) -> Result<PathBuf> {
        let path = if matches!(record.kind, MaterialKind::GitRepository { .. }) {
            format!("source/{}/repo", record.source_uuid)
        } else {
            format!(
                "source/{}/retrieval/{}/material",
                record.source_uuid, record.uuid
            )
        };
        Ok(beneath(&self.root, &path)?)
    }

    pub(super) fn verify_retrieval(
        &self,
        record: &RetrievalRecord,
        request: &RetrieveRequest,
    ) -> Result<()> {
        ensure!(
            record.state == RetrievalState::Ready,
            Error::new(ErrorCode::ContentUnavailable, "retrieval was purged")
        );
        let path = self.retrieval_path(record)?;
        crate::utils::path::plain_metadata(&path)?;
        if let Resolved::Git { commit } = &record.resolved {
            let Source::Git { origin } = normalize(self.source(record.source_uuid)?.source)? else {
                bail!("Git retrieval source mismatch")
            };
            let scratch = tempfile::tempdir()?;
            let inventory = Git::default().inspect_offline(
                RepositoryMaterial {
                    origin: &origin,
                    repository: &path,
                    commit,
                    dependencies: path.parent().unwrap(),
                },
                scratch.path(),
            )?;
            ensure!(
                inventory.dependencies == record.dependencies,
                Error::new(ErrorCode::IntegrityError, "retrieval dependencies changed")
            );
        } else {
            let directory = path.parent().unwrap();
            ensure!(
                crate::core::store::manifest(directory)? == record.files,
                Error::new(ErrorCode::IntegrityError, "retrieval content changed")
            );
            if matches!(record.kind, MaterialKind::File) {
                let upstream = match &record.resolved {
                    Resolved::Release { assets, .. } => {
                        assets.first().and_then(|asset| asset.digest.as_deref())
                    }
                    _ => None,
                };
                let expected = retrieve::expected_digest(request, upstream)?;
                retrieve::verify_digest(
                    expected.as_deref(),
                    record
                        .files
                        .get("material")
                        .and_then(|entry| entry.digest.as_deref()),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn cache_hit(
        &self,
        record: &RetrievalRecord,
        request: &RetrieveRequest,
    ) -> Result<bool> {
        match self.verify_retrieval(record, request) {
            Ok(()) => Ok(true),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn retrieved_handle(
        &self,
        record: &RetrievalRecord,
        request: &RetrieveRequest,
        guard: LockGuard,
        reused: bool,
        stale: bool,
        origin_checked: bool,
    ) -> Result<RetrievedResource> {
        let mut verification = record.verification.clone();
        if matches!(record.kind, MaterialKind::File) {
            let upstream = match &record.resolved {
                Resolved::Release { assets, .. } => {
                    assets.first().and_then(|asset| asset.digest.as_deref())
                }
                _ => None,
            };
            let expected = retrieve::expected_digest(request, upstream)?;
            verification.expected_sha256 = retrieve::verify_digest(
                expected.as_deref(),
                record
                    .files
                    .get("material")
                    .and_then(|entry| entry.digest.as_deref()),
            )?
            .expected_sha256;
        }
        Ok(RetrievedResource {
            request: request.clone(),
            source: self.source(record.source_uuid)?.source,
            store: Some(self.clone()),
            descriptor: RetrievedDescriptor {
                ownership: Ownership::Managed { uuid: record.uuid },
                path: self.retrieval_path(record)?,
                kind: record.kind.clone(),
                source: SourceInput::Registered {
                    source_uuid: record.source_uuid,
                },
                resolved: record.resolved.clone(),
                dependencies: record.dependencies.clone(),
                verification,
                checked_at: Some(record.checked_at),
                origin_checked,
                reused,
                stale,
            },
            _temporary: None,
            _coordination: Some(guard),
        })
    }

    pub fn configure_source_freshness(
        &self,
        id: Uuid,
        freshness: Freshness,
    ) -> Result<SourceRecord> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "source configuration requires administration"
            )
        );
        let _source = LockGuard::acquire(
            &self.root.join(format!(".locks/source-{id}")),
            LockMode::Exclusive,
            Duration::from_secs(150),
        )?;
        let _lock = self.lock()?;
        let mut source = self.source(id)?;
        source.retrieval_freshness = Some(freshness);
        self.save_source(&source)?;
        Ok(source)
    }

    /// Returns false when live source users defer retirement. Retry explicitly.
    pub fn purge_retrieval(&self, id: Uuid) -> Result<bool> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "retrieval purge requires administration"
            )
        );
        let mut record = self.retrieval_record(id)?;
        let guard = LockGuard::acquire(
            &self
                .root
                .join(format!(".locks/source-{}", record.source_uuid)),
            LockMode::Exclusive,
            Duration::ZERO,
        );
        let _guard = match guard {
            Ok(guard) => guard,
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let _lock = self.lock()?;
        record = self.retrieval_record(id)?;
        if record.state == RetrievalState::Purged {
            return Ok(true);
        }
        record.state = RetrievalState::PurgePending;
        self.commit(Journal {
            records: vec![Record::Retrieval(Box::new(record.clone()))],
            ..Default::default()
        })?;
        if !matches!(record.kind, MaterialKind::GitRepository { .. }) {
            let path = beneath(
                &self.root,
                &format!("source/{}/retrieval/{}", record.source_uuid, id),
            )?;
            match fs::remove_dir_all(path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        record.state = RetrievalState::Purged;
        self.commit(Journal {
            records: vec![Record::Retrieval(Box::new(record))],
            ..Default::default()
        })?;
        Ok(true)
    }
}
