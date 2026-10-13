use crate::{
    core::{
        Store,
        index::Journal,
        source::AcquireOptions,
        store::{json_digest, manifest},
    },
    error::{Error, ErrorCode},
    models::{
        claim::{
            Claimed, CoveredView,
            policy::{Action, ReclaimStrategy},
        },
        index::Record,
        store::Acquired,
    },
};
use anyhow::{Context, Result, ensure};
use uuid::Uuid;

#[cfg(test)]
mod tests;

impl Store {
    /// Explicit recovery follows the claim's registered effective policy. Reads
    /// and ordinary leases never enter this path.
    pub fn recover_claim(&self, app: Uuid, id: Uuid, options: &AcquireOptions) -> Result<Claimed> {
        self.recover_claim_view(app, id, None, None, options)
    }

    pub(crate) fn recover_claim_view(
        &self,
        app: Uuid,
        id: Uuid,
        expected: Option<u64>,
        relative: Option<String>,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        let (revision, claim, original) = {
            let _lock = self.lock()?;
            let application = self.application(app)?;
            ensure!(
                expected.is_none_or(|revision| revision == application.revision),
                Error::new(ErrorCode::RevisionConflict, "claim changed before recovery")
            );
            let registry = self.registry(app)?;
            self.check_session(&registry)?;
            let claim = registry
                .claims
                .get(&id)
                .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?
                .clone();
            self.authorize_claim(&registry, &claim, Action::Recover)?;
            if self.policy_fence.is_none() {
                let execution = self.with_policy_fence(claim.request.source_uuid, Some(app))?;
                drop(_lock);
                return execution.recover_claim_view(app, id, expected, relative, options);
            }
            self.check_policy_fence()?;
            let original = self.output(claim.entry_uuid)?;
            match self.metadata_lease_locked(&original) {
                Ok(mut lease) => {
                    drop(_lock);
                    lease.verify(self, &original)?;
                    let _lock = self.lock()?;
                    let current = self.registry(app)?;
                    self.authorize_claim(&current, &claim, Action::Recover)?;
                    ensure!(
                        self.application(app)?.revision == application.revision,
                        Error::new(
                            ErrorCode::RevisionConflict,
                            "application changed while validating recovery"
                        )
                    );
                    lease.project(&original, relative.as_deref())?;
                    return Ok(Claimed {
                        view: CoveredView {
                            app,
                            claim: id,
                            entry: original.uuid,
                            relative,
                        },
                        claim,
                        acquired: Acquired {
                            directory: lease.directory().to_owned(),
                            entry: original,
                            reused: true,
                            update_checked: false,
                            stale: false,
                            reclaim: Default::default(),
                        },
                    });
                }
                Err(error) if Error::from_anyhow(&error).code == ErrorCode::ContentUnavailable => {}
                Err(error) => return Err(error),
            }
            ensure!(
                claim.effective.reclaim.strategy() != ReclaimStrategy::ReportUnavailable,
                Error::new(
                    ErrorCode::ContentUnavailable,
                    "selected claim content is unavailable"
                )
            );
            (application.revision, claim, original)
        };
        let (mut acquired, mut lease) = match claim.effective.reclaim.strategy() {
            ReclaimStrategy::ReportUnavailable => unreachable!(),
            ReclaimStrategy::Refresh => self.acquire_candidate(&claim.request, options)?,
            ReclaimStrategy::RestoreExact => {
                let mut replay = original.clone();
                replay.request.adopted = claim.request.adopted.clone();
                let prepared = self.prepare_exact(&replay, options).map_err(|error| {
                    if matches!(
                        Error::from_anyhow(&error).code,
                        ErrorCode::IntegrityError | ErrorCode::KeyUnavailable
                    ) {
                        error
                    } else {
                        error.context(Error::new(
                            ErrorCode::ExactContentUnavailable,
                            "original source content is unavailable",
                        ))
                    }
                })?;
                let files = manifest(&prepared.temp.path().join("content"))?;
                ensure!(
                    files == original.files
                        && json_digest(&files)? == original.content_digest
                        && prepared.resolved == original.resolved
                        && prepared.dependencies == original.dependencies,
                    Error::new(
                        ErrorCode::IntegrityError,
                        "restored content or dependencies differ from saved provenance"
                    )
                );
                let result = self.publish_prepared(&claim.request, prepared, None, true, true)?;
                (
                    result.acquired,
                    result.lease.expect("recovery candidate lease"),
                )
            }
        };
        lease.verify(self, &acquired.entry)?;
        lease.project(&acquired.entry, relative.as_deref())?;
        acquired.directory = lease.directory().to_owned();
        #[cfg(test)]
        if let Some(observer) = &self.before_claim_commit {
            observer();
        }
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        self.check_session(&registry)?;
        self.authorize_claim(&registry, &claim, Action::Recover)?;
        ensure!(
            application.revision == revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "application changed during recovery"
            )
        );
        let mut updated = claim;
        updated.entry_uuid = acquired.entry.uuid;
        registry.claims.insert(id, updated.clone());
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            registries: vec![registry],
            ..Default::default()
        })?;
        acquired.reclaim = self.reclaim_locked()?;
        Ok(Claimed {
            view: CoveredView {
                app,
                claim: id,
                entry: acquired.entry.uuid,
                relative,
            },
            claim: updated,
            acquired,
        })
    }
}
