use super::matching;
use crate::{
    core::{Store, index::Journal, source::AcquireOptions, store::ResourceLease},
    error::{Error, ErrorCode},
    models::{
        claim::{Claim, Claimed, CoveredView, EnsureRequest, policy::Action},
        index::Record,
        policy::Identical,
        store::{Acquired, Request},
    },
};
use anyhow::{Result, ensure};
use uuid::Uuid;

#[cfg(test)]
mod tests;

struct Selected {
    claim: Claim,
    relative: Option<String>,
    acquired: Acquired,
    lease: ResourceLease,
    revision: u64,
}

impl Store {
    /// Ensure one ownership record per compatible scoped intent. Preparation is
    /// outside metadata coordination; final publication rechecks current state.
    pub fn ensure_claim(
        &self,
        app: Uuid,
        input: &EnsureRequest,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        crate::models::claim::validate_context(&input.context)?;
        let (revision, existing, request, effective) = {
            let _lock = self.lock()?;
            let application = self.application(app)?;
            let registry = self.registry(app)?;
            self.authorize_resource(
                &registry,
                Action::Acquire,
                Some(input.source_uuid),
                Some(&input.scope),
            )?;
            let effective = registry.registry.policies.evaluate(&input.overrides)?;
            let request = Request {
                source_uuid: input.source_uuid,
                selector: input.selector.clone(),
                selection: input.selection.clone(),
                policies: effective.output.clone(),
            };
            crate::core::policy::validate(&request)?;
            let existing = matching::select(self, &registry, &input.scope, &request, &effective)?;
            if let Some(existing) = &existing {
                ensure!(
                    existing.claim.context == input.context,
                    Error::new(
                        ErrorCode::ClaimContextConflict,
                        "existing claim context differs; use revision-checked context replacement"
                    )
                );
                let entry = self.output(existing.claim.entry_uuid)?;
                if let Err(error) = self.metadata_path(&entry) {
                    if Error::from_anyhow(&error).code != ErrorCode::ContentUnavailable {
                        return Err(error);
                    }
                    drop(_lock);
                    return self.recover_claim_view(
                        app,
                        existing.claim.uuid,
                        Some(application.revision),
                        existing.relative.clone(),
                        options,
                    );
                }
                if effective.output.identical == Identical::Reuse {
                    let selected = match self.select_existing(existing, application.revision) {
                        Ok(selected) => selected,
                        Err(error)
                            if Error::from_anyhow(&error).code == ErrorCode::ContentUnavailable =>
                        {
                            drop(_lock);
                            return self.recover_claim_view(
                                app,
                                existing.claim.uuid,
                                Some(application.revision),
                                existing.relative.clone(),
                                options,
                            );
                        }
                        Err(error) => return Err(error),
                    };
                    drop(_lock);
                    return self.finish_ensure(app, selected);
                }
                self.authorize_claim(&registry, &existing.claim, Action::Update)?;
            }
            // Refreshing a covered view must prepare its entire owning request.
            let prepared_request = existing
                .as_ref()
                .map(|matched| {
                    let mut original = matched.claim.request.clone();
                    original.policies = request.policies.clone();
                    original
                })
                .unwrap_or(request);
            (application.revision, existing, prepared_request, effective)
        };
        let (mut acquired, mut lease) = self.acquire_candidate(&request, options)?;
        lease.verify(self, &acquired.entry)?;
        #[cfg(test)]
        if let Some(observer) = &self.before_claim_commit {
            observer();
        }
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        self.authorize_resource(
            &registry,
            Action::Acquire,
            Some(input.source_uuid),
            Some(&input.scope),
        )?;
        ensure!(
            registry.registry.policies.evaluate(&input.overrides)? == effective,
            Error::new(
                ErrorCode::RevisionConflict,
                "registered policy changed during acquisition"
            )
        );
        // Recheck the original requested projection, not its owning broader request.
        let projected_request = Request {
            source_uuid: input.source_uuid,
            selector: input.selector.clone(),
            selection: input.selection.clone(),
            policies: effective.output.clone(),
        };
        let current = matching::select(
            self,
            &registry,
            &input.scope,
            &projected_request,
            &effective,
        )?;
        if effective.output.identical == Identical::NewEntry
            && let Some(current) = &current
        {
            self.authorize_claim(&registry, &current.claim, Action::Update)?;
        }
        if application.revision != revision {
            if effective.output.identical == Identical::Reuse
                && let Some(current) = current
            {
                ensure!(
                    current.claim.context == input.context,
                    Error::new(
                        ErrorCode::ClaimContextConflict,
                        "existing claim context differs"
                    )
                );
                let selected = self.select_existing(&current, application.revision)?;
                drop(_lock);
                return self.finish_ensure(app, selected);
            }
            return Err(Error::new(
                ErrorCode::RevisionConflict,
                "application changed during acquisition; retry",
            )
            .into());
        }
        let (id, relative, mut overrides) = if let Some(existing) = existing {
            (
                existing.claim.uuid,
                existing.relative,
                existing.claim.overrides,
            )
        } else {
            (Uuid::new_v4(), None, input.overrides.clone())
        };
        // An explicit new_entry call can advance the same claim without changing
        // retention/recovery ownership. Persist the requested acquisition mode.
        if effective.output.identical == Identical::NewEntry {
            overrides.identical = Some(Identical::NewEntry);
        }
        lease.project(&acquired.entry, relative.as_deref())?;
        acquired.directory = lease.directory().to_path_buf();
        let claim = Claim {
            uuid: id,
            scope: input.scope.clone(),
            request,
            entry_uuid: acquired.entry.uuid,
            context: input.context.clone(),
            overrides,
            effective,
        };
        registry.claims.insert(id, claim.clone());
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application.clone())],
            registries: vec![registry],
            ..Default::default()
        })?;
        acquired.reclaim = self.reclaim_locked()?;
        Ok(Claimed {
            view: CoveredView {
                app,
                claim: claim.uuid,
                entry: claim.entry_uuid,
                relative,
            },
            claim,
            acquired,
        })
    }

    fn select_existing(&self, selected: &matching::Match, revision: u64) -> Result<Selected> {
        let entry = self.output(selected.claim.entry_uuid)?;
        let lease = self.metadata_lease_locked(&entry)?;
        let acquired = Acquired {
            directory: lease.directory().to_path_buf(),
            entry,
            reused: true,
            update_checked: false,
            stale: false,
            reclaim: Default::default(),
        };
        Ok(Selected {
            claim: selected.claim.clone(),
            relative: selected.relative.clone(),
            acquired,
            lease,
            revision,
        })
    }

    fn finish_ensure(&self, app: Uuid, mut selected: Selected) -> Result<Claimed> {
        selected.lease.verify(self, &selected.acquired.entry)?;
        let _lock = self.lock()?;
        let application = self.application(app)?;
        let registry = self.registry(app)?;
        self.authorize_claim(&registry, &selected.claim, Action::Acquire)?;
        ensure!(
            application.revision == selected.revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "application changed while validating the selected output"
            )
        );
        selected
            .lease
            .project(&selected.acquired.entry, selected.relative.as_deref())?;
        selected.acquired.directory = selected.lease.directory().to_path_buf();
        Ok(Claimed {
            view: CoveredView {
                app,
                claim: selected.claim.uuid,
                entry: selected.claim.entry_uuid,
                relative: selected.relative,
            },
            claim: selected.claim,
            acquired: selected.acquired,
        })
    }

    pub fn lease_view(
        &self,
        view: &CoveredView,
        mode: crate::models::store::VerificationMode,
    ) -> Result<ResourceLease> {
        view.validate()?;
        let mut lease = self.lease_claim(view.app, view.claim, mode)?;
        let output = self.output(lease.entry_uuid())?;
        lease.project(&output, view.relative.as_deref())?;
        Ok(lease)
    }
}
