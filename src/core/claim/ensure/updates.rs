//! Automatic ensure uses existing ownership matching and publication fences.
use super::*;
use crate::{
    core::{
        claim::operations::ClaimTarget,
        retrieval::RetrieveOptions,
        store::{json_digest, updates::scoped_intent_key},
    },
    models::{
        policy::Freshness,
        retrieval::{CachePolicy, Ownership},
    },
};

impl Store {
    pub(super) fn ensure_updated(
        &self,
        app: Uuid,
        input: &EnsureRequest,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        // Covering and covered requests share this lock. Their final state remains
        // keyed by claim identity, preserving independent explicit scopes.
        let operation_key = json_digest(&(
            "claim-update",
            app,
            &input.scope,
            input.source_uuid,
            &input.selector,
        ))?;
        let _intent = self.acquisition_lock(&operation_key)?;
        let (existing, request, effective, mut authorization) = {
            let _lock = self.lock()?;
            let registry = self.registry(app)?;
            let effective = self.claim_execution_policy(
                input.source_uuid,
                &registry.registry,
                Some(input.adopted.as_ref()),
            )?;
            let mut request = OutputPlan {
                adopted: Some(input.adopted.clone()),
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
                        "existing claim context differs; use context replacement"
                    )
                );
                request = existing.claim.request.clone();
                request.policies = effective.output.clone();
            }
            let authorization =
                self.begin_composition(app, Some(&input.scope), input.source_uuid, None)?;
            (existing, request, effective, authorization)
        };
        let claim_id = existing
            .as_ref()
            .map_or_else(Uuid::new_v4, |existing| existing.claim.uuid);
        authorization.owner.claim = Some(claim_id);
        let (mut retrieval, mut acquisition) = self.composition_requests(&request, options)?;
        acquisition.update_policy = effective.update_policy;
        let key = scoped_intent_key(&retrieval, &acquisition, Some(&authorization))?;
        let previous = self.acquisition_by_key(&key)?;
        let selected = match &existing {
            Some(matched) => match self.select_existing(matched, authorization.revision) {
                Ok(selected) => Some(selected),
                Err(error) if Error::from_anyhow(&error).code == ErrorCode::ContentUnavailable => {
                    return self.recover_claim_view(
                        app,
                        matched.claim.uuid,
                        Some(authorization.revision),
                        matched.relative.clone(),
                        options,
                    );
                }
                Err(error) => return Err(error),
            },
            None => None,
        };
        let last_success = previous
            .as_ref()
            .filter(|state| {
                existing
                    .as_ref()
                    .is_some_and(|existing| existing.claim.entry_uuid == state.selected)
            })
            .and_then(|state| state.last_success);
        let due =
            options.force_refresh || acquisition.update_policy.due(last_success, options.now()?);
        authorization.update =
            existing.is_some() && (due || acquisition.identical == Identical::NewEntry);
        authorization.check(self, input.source_uuid, Some(&acquisition))?;
        if !due
            && acquisition.identical == Identical::Reuse
            && let Some(selected) = selected
        {
            return self.finish_ensure(app, selected);
        }
        if let CachePolicy::Cached { freshness, .. } = &mut retrieval.cache
            && freshness.is_none()
        {
            *freshness = self.source(input.source_uuid)?.retrieval_freshness;
        }
        crate::core::policy::composition::validate_composition(&retrieval, &acquisition, due)?;
        if due && let CachePolicy::Cached { freshness, .. } = &mut retrieval.cache {
            *freshness = Some(Freshness::Always {});
        }
        let retrieved = self.retrieve_authorized(
            &retrieval,
            &RetrieveOptions {
                test_now: options.test_now,
                test_github_api: options.test_github_api.clone(),
            },
            Some(authorization.clone()),
        )?;
        if retrieved.descriptor().stale
            && let Some(mut selected) = selected
        {
            selected.acquired.stale = true;
            return self.finish_ensure(app, selected);
        }
        let relative = existing
            .as_ref()
            .and_then(|existing| existing.relative.clone());
        let target = ClaimTarget {
            app,
            scope: input.scope.clone(),
            id: claim_id,
            expected: existing.as_ref().map(|existing| existing.claim.entry_uuid),
            context: input.context.clone(),
            expected_revision: Some(authorization.revision),
            projection: relative.clone(),
        };
        let result = self.publish_acquisition(
            &retrieved,
            &acquisition,
            key,
            previous.as_ref(),
            Some(target),
        )?;
        let Ownership::Managed { uuid } = result.descriptor().ownership else {
            unreachable!()
        };
        // Return the publication's identity even if another writer advances the
        // live claim afterwards. Its descriptor and content are immutable.
        let entry = self.output(uuid)?;
        let claim = Claim {
            policy_refs: existing
                .as_ref()
                .map(|existing| existing.claim.policy_refs.clone())
                .map(Ok)
                .unwrap_or_else(|| self.adopted_refs(Some(input.adopted.as_ref())))?,
            uuid: claim_id,
            scope: input.scope.clone(),
            request,
            entry_uuid: uuid,
            context: input.context.clone(),
            effective,
        };
        let directory = match &relative {
            Some(relative) => crate::utils::path::beneath(result.path(), relative)?,
            None => result.path().to_owned(),
        };
        Ok(Claimed {
            view: CoveredView {
                app,
                claim: claim_id,
                entry: uuid,
                relative,
            },
            claim,
            acquired: Acquired {
                entry,
                directory,
                reused: result.descriptor().reused,
                update_checked: retrieved.descriptor().origin_checked,
                stale: result.descriptor().stale,
                reclaim: result.descriptor().reclaim.clone().unwrap_or_default(),
            },
        })
    }
}
