use super::Journal;
use crate::{
    core::{Store, source::AcquireOptions},
    models::{
        index::{Claim, ClaimView, Claimed, Record, Settings},
        store::{Acquired, Request},
    },
};
use anyhow::{Context, Result, ensure};
use uuid::Uuid;

pub(crate) struct ClaimTarget {
    pub app: Uuid,
    pub id: Uuid,
    pub expected: Option<Uuid>,
}
impl Store {
    pub fn acquire_for(
        &self,
        app: Uuid,
        request: &Request,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.application(app)?;
        let id = Uuid::new_v4();
        let acquired = self.acquire_inner(
            request,
            options,
            Some(ClaimTarget {
                app,
                id,
                expected: None,
            }),
        )?;
        Ok(scoped(id, request, acquired))
    }
    pub fn update_claim(&self, app: Uuid, id: Uuid, options: &AcquireOptions) -> Result<Claimed> {
        let application = self.application(app)?;
        let claim = application.claims.get(&id).context("claim missing")?;
        let acquired = self.acquire_inner(
            &claim.request,
            options,
            Some(ClaimTarget {
                app,
                id,
                expected: Some(claim.entry_uuid),
            }),
        )?;
        Ok(scoped(id, &claim.request, acquired))
    }
    pub fn release_claim(&self, app: Uuid, id: Uuid) -> Result<()> {
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        application.claims.remove(&id).context("claim missing")?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            stage: None,
        })
    }
    pub fn set_settings(&self, app: Uuid, settings: Settings) -> Result<()> {
        for source in &settings.source_filter {
            self.source(*source)?;
        }
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        application.settings = settings;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            stage: None,
        })
    }
    /// Filters only this application's existing claims. Settings are not grants.
    pub fn claims(&self, app: Uuid) -> Result<Vec<ClaimView>> {
        let _lock = self.lock()?;
        let application = self.application(app)?;
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
            "claim changed during acquisition; retry"
        );
        app.claims.insert(
            target.id,
            Claim {
                uuid: target.id,
                request: request.clone(),
                entry_uuid: entry,
            },
        );
        Ok(Record::Application(app))
    }
}
fn scoped(id: Uuid, request: &Request, acquired: Acquired) -> Claimed {
    Claimed {
        claim: Claim {
            uuid: id,
            request: request.clone(),
            entry_uuid: acquired.entry.uuid,
        },
        acquired,
    }
}
