use super::registry::RegistryState;
use crate::core::index::Journal;
use crate::error::{Error, ErrorCode};
use crate::models::claim::{
    Scope,
    policy::{Action, Overrides},
};
use crate::{
    core::{Store, source::AcquireOptions},
    models::{
        index::{
            Application, Claim, ClaimContext, ClaimView, Claimed, Record, Settings,
            validate_context,
        },
        store::Request,
    },
};
use anyhow::{Context, Result, ensure};
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub(crate) struct ClaimTarget {
    pub app: Uuid,
    pub scope: Scope,
    pub id: Uuid,
    pub expected: Option<Uuid>,
    pub context: ClaimContext,
    pub expected_revision: Option<u64>,
    pub overrides: Option<Overrides>,
    pub projection: Option<String>,
}
impl Store {
    pub fn acquire_for(
        &self,
        app: Uuid,
        scope: &Scope,
        request: &Request,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.acquire_for_with_context(app, scope, request, &ClaimContext::new(), options)
    }
    /// Publish caller context atomically with a new claim, independent of output reuse.
    pub fn acquire_for_with_context(
        &self,
        app: Uuid,
        scope: &Scope,
        request: &Request,
        context: &ClaimContext,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.ensure_compiled(
            app,
            &crate::models::claim::CompiledEnsure {
                adopted: None,
                scope: scope.clone(),
                source_uuid: request.source_uuid,
                selector: request.selector.clone(),
                selection: request.selection.clone(),
                context: context.clone(),
                overrides: request_overrides(request),
            },
            options,
        )
    }
    pub fn update_claim(&self, app: Uuid, id: Uuid, options: &AcquireOptions) -> Result<Claimed> {
        self.application(app)?;
        let registry = self.registry(app)?;
        self.check_session(&registry)?;
        let claim = registry
            .claims
            .get(&id)
            .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
        self.authorize_claim(&registry, claim, Action::Update)?;
        if self.policy_fence.is_none() {
            return self
                .with_policy_fence(claim.request.source_uuid, Some(app))?
                .update_claim(app, id, options);
        }
        let acquired = self.acquire_inner(
            &claim.request,
            options,
            Some(ClaimTarget {
                app,
                scope: claim.scope.clone(),
                id,
                expected: Some(claim.entry_uuid),
                context: Default::default(),
                expected_revision: None,
                overrides: None,
                projection: None,
            }),
        )?;
        let view = crate::models::claim::CoveredView {
            app,
            claim: id,
            entry: acquired.acquired.entry.uuid,
            relative: None,
        };
        Ok(Claimed {
            view,
            claim: acquired
                .claim
                .expect("scoped acquisition publishes a claim"),
            acquired: acquired.acquired,
        })
    }
    /// Replace all labels with optimistic application-revision coordination.
    pub fn set_claim_context(
        &self,
        app: Uuid,
        id: Uuid,
        context: &ClaimContext,
        expected_revision: u64,
    ) -> Result<Claim> {
        validate_context(context)?;
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        self.check_session(&registry)?;
        ensure!(
            application.revision == expected_revision,
            Error::new(ErrorCode::RevisionConflict, "application revision conflict")
        );
        self.authorize_claim(
            &registry,
            registry
                .claims
                .get(&id)
                .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?,
            Action::SetContext,
        )?;
        let claim = registry
            .claims
            .get_mut(&id)
            .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
        if claim.context == *context {
            return Ok(claim.clone());
        }
        claim.context = context.clone();
        let result = claim.clone();
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            registries: vec![registry],
            ..Default::default()
        })?;
        Ok(result)
    }
    pub fn release_claim(&self, app: Uuid, id: Uuid) -> Result<()> {
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        self.check_session(&registry)?;
        self.authorize_claim(
            &registry,
            registry
                .claims
                .get(&id)
                .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?,
            Action::Release,
        )?;
        registry
            .claims
            .remove(&id)
            .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            registries: vec![registry],
            stage: None,
            ..Default::default()
        })
    }
    pub fn set_settings(&self, app: Uuid, settings: Settings) -> Result<()> {
        for source in &settings.source_filter {
            self.source(*source)?;
        }
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        if application.settings.source_filter == settings.source_filter {
            return Ok(());
        }
        application.settings = settings;
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            stage: None,
            ..Default::default()
        })
    }
    /// Filters only this application's existing claims. Settings are not grants.
    pub fn claims(&self, app: Uuid) -> Result<Vec<ClaimView>> {
        let _lock = self.lock()?;
        let application = self.application(app)?;
        self.claim_views_locked(application, |_| true)
    }
    /// Caller holds the mutation lock for the application and output snapshot.
    pub(crate) fn claim_views_locked(
        &self,
        application: Application,
        matches: impl Fn(&Claim) -> bool,
    ) -> Result<Vec<ClaimView>> {
        let registry = self.registry(application.uuid)?;
        self.check_session(&registry)?;
        registry
            .claims
            .values()
            .filter(|claim| {
                self.authorize_claim(&registry, claim, Action::Read).is_ok()
                    && self
                        .authorize_claim(&registry, claim, Action::Discover)
                        .is_ok()
            })
            .filter(|c| {
                application.settings.source_filter.is_empty()
                    || application
                        .settings
                        .source_filter
                        .contains(&c.request.source_uuid)
            })
            .filter(|claim| matches(claim))
            .cloned()
            .map(|claim| {
                let state = self.output(claim.entry_uuid)?.state;
                Ok(ClaimView { claim, state })
            })
            .collect()
    }
    pub(crate) fn claim_record(
        &self,
        target: &ClaimTarget,
        request: &Request,
        entry: Uuid,
    ) -> Result<(Record, RegistryState)> {
        let mut app = self.application(target.app)?;
        ensure!(
            target
                .expected_revision
                .is_none_or(|expected| expected == app.revision),
            Error::new(
                ErrorCode::RevisionConflict,
                "application changed during acquisition"
            )
        );
        let mut registry = self.registry(target.app)?;
        self.authorize_resource(
            &registry,
            if target.expected.is_some() {
                Action::Update
            } else {
                Action::Acquire
            },
            Some(request.source_uuid),
            Some(&target.scope),
        )?;
        ensure!(
            registry.claims.get(&target.id).map(|c| c.entry_uuid) == target.expected,
            Error::new(
                ErrorCode::RevisionConflict,
                "claim changed during acquisition; retry"
            )
        );
        let context = registry
            .claims
            .get(&target.id)
            .map(|claim| claim.context.clone())
            .unwrap_or_else(|| target.context.clone());
        let overrides = target
            .overrides
            .clone()
            .or_else(|| {
                registry
                    .claims
                    .get(&target.id)
                    .map(|claim| claim.overrides.clone())
            })
            .unwrap_or_else(|| request_overrides(request));
        let effective = registry.registry.policies.evaluate(&overrides)?;
        let policy_refs = registry
            .claims
            .get(&target.id)
            .map(|claim| claim.policy_refs.clone())
            .map(Ok)
            .unwrap_or_else(|| self.adopted_refs(request.adopted.as_deref()))?;
        registry.claims.insert(
            target.id,
            Claim {
                policy_refs,
                uuid: target.id,
                scope: target.scope.clone(),
                overrides,
                effective,
                request: request.clone(),
                entry_uuid: entry,
                context,
            },
        );
        app.bump()?;
        registry.registry.revision = app.revision;
        registry.validate()?;
        Ok((Record::Application(app), registry))
    }
}

fn request_overrides(request: &Request) -> Overrides {
    Overrides {
        identical: Some(request.policies.identical),
        revision: Some(request.policies.revision),
        reclaim: None,
        useable_app: Some(request.policies.protection.is_some()),
        update_policy: None,
    }
}
