use super::Journal;
use crate::error::{Error, ErrorCode};
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
    pub id: Uuid,
    pub expected: Option<Uuid>,
    pub context: ClaimContext,
}
impl Store {
    pub fn acquire_for(
        &self,
        app: Uuid,
        request: &Request,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.acquire_for_with_context(app, request, &ClaimContext::new(), options)
    }
    /// Publish caller context atomically with a new claim, independent of output reuse.
    pub fn acquire_for_with_context(
        &self,
        app: Uuid,
        request: &Request,
        context: &ClaimContext,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        validate_context(context)?;
        self.application(app)?;
        let id = Uuid::new_v4();
        let acquired = self.acquire_inner(
            request,
            options,
            Some(ClaimTarget {
                app,
                id,
                expected: None,
                context: context.clone(),
            }),
        )?;
        Ok(Claimed {
            claim: acquired
                .claim
                .expect("scoped acquisition publishes a claim"),
            acquired: acquired.acquired,
        })
    }
    pub fn update_claim(&self, app: Uuid, id: Uuid, options: &AcquireOptions) -> Result<Claimed> {
        let application = self.application(app)?;
        let claim = application
            .claims
            .get(&id)
            .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
        let acquired = self.acquire_inner(
            &claim.request,
            options,
            Some(ClaimTarget {
                app,
                id,
                expected: Some(claim.entry_uuid),
                context: Default::default(),
            }),
        )?;
        Ok(Claimed {
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
        ensure!(
            application.revision == expected_revision,
            Error::new(ErrorCode::RevisionConflict, "application revision conflict")
        );
        let claim = application
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
            ..Default::default()
        })?;
        Ok(result)
    }
    pub fn release_claim(&self, app: Uuid, id: Uuid) -> Result<()> {
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        application
            .claims
            .remove(&id)
            .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
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
        application
            .claims
            .into_values()
            .filter(|c| {
                application.settings.source_filter.is_empty()
                    || application
                        .settings
                        .source_filter
                        .contains(&c.request.source_uuid)
            })
            .filter(matches)
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
    ) -> Result<Record> {
        let mut app = self.application(target.app)?;
        ensure!(
            app.claims.get(&target.id).map(|c| c.entry_uuid) == target.expected,
            Error::new(
                ErrorCode::RevisionConflict,
                "claim changed during acquisition; retry"
            )
        );
        let context = app
            .claims
            .get(&target.id)
            .map(|claim| claim.context.clone())
            .unwrap_or_else(|| target.context.clone());
        app.claims.insert(
            target.id,
            Claim {
                uuid: target.id,
                request: request.clone(),
                entry_uuid: entry,
                context,
            },
        );
        app.bump()?;
        Ok(Record::Application(app))
    }
}
