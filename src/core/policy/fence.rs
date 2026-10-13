//! Narrow attachment observations avoid conflicts from unrelated catalog writes.
use crate::{
    core::Store,
    error::{Error, ErrorCode},
    models::policy::PolicyRefs,
};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PolicyFence {
    source: Option<(Uuid, u64, PolicyRefs)>,
    application: Option<(Uuid, Arc<AtomicU64>, PolicyRefs)>,
    definitions: BTreeMap<Uuid, String>,
    authority: Option<(
        crate::models::policy::effective::EvaluatedPolicies,
        Option<crate::models::claim::Scope>,
    )>,
}
impl PolicyFence {
    pub fn capture(store: &Store, source: Option<Uuid>, app: Option<Uuid>) -> Result<Self> {
        Self::capture_references(store, source, app, &[])
    }
    pub(crate) fn capture_references(
        store: &Store,
        source: Option<Uuid>,
        app: Option<Uuid>,
        request: &[Uuid],
    ) -> Result<Self> {
        let _lock = store.lock()?;
        let mut fence = Self {
            source: source
                .map(|uuid| {
                    let source = store.source(uuid)?;
                    Ok::<_, anyhow::Error>((uuid, source.policy_revision, source.policy_refs))
                })
                .transpose()?,
            application: app
                .map(|uuid| {
                    let registry = store.registry(uuid)?;
                    Ok::<_, anyhow::Error>((
                        uuid,
                        Arc::new(AtomicU64::new(registry.registry.revision)),
                        registry.registry.policy_refs,
                    ))
                })
                .transpose()?,
            definitions: BTreeMap::new(),
            authority: None,
        };
        let snapshot = store.metadata.snapshot()?;
        let catalog = super::persistence::load(&store.metadata, &snapshot)?;
        for refs in fence
            .source
            .iter()
            .map(|(_, _, refs)| refs)
            .chain(fence.application.iter().map(|(_, _, refs)| refs))
        {
            for uuid in refs.ids() {
                fence
                    .definitions
                    .insert(*uuid, crate::core::store::json_digest(catalog.get(*uuid)?)?);
            }
        }
        for uuid in request {
            fence
                .definitions
                .insert(*uuid, crate::core::store::json_digest(catalog.get(*uuid)?)?);
        }
        Ok(fence)
    }
    pub(crate) fn with_authority(
        mut self,
        plan: crate::models::policy::effective::EvaluatedPolicies,
        scope: Option<&crate::models::claim::Scope>,
    ) -> Self {
        self.authority = Some((plan, scope.cloned()));
        self
    }
    fn authorize(&self, action: crate::models::claim::policy::Action) -> Result<()> {
        if let Some((plan, scope)) = &self.authority {
            super::resolve::authorize(
                plan,
                action,
                self.source.as_ref().map(|(id, _, _)| *id),
                scope.as_ref(),
            )?;
        }
        Ok(())
    }
    pub fn check(&self, store: &Store) -> Result<()> {
        let _lock = store.lock()?;
        // A snapshot verifies referenced definition bytes even if no attachment
        // revision changed. Definition overwrite/deletion is prohibited.
        let snapshot = store.metadata.snapshot()?;
        let catalog = super::persistence::load(&store.metadata, &snapshot)?;
        for (uuid, expected) in &self.definitions {
            ensure!(
                &crate::core::store::json_digest(catalog.get(*uuid)?)? == expected,
                Error::new(
                    ErrorCode::RevisionConflict,
                    "policy definition changed during operation"
                )
            );
        }
        if let Some((uuid, revision, refs)) = &self.source {
            let current = store.source(*uuid)?;
            ensure!(
                current.policy_revision == *revision && current.policy_refs == *refs,
                Error::new(
                    ErrorCode::RevisionConflict,
                    "source policy associations changed"
                )
            );
        }
        if let Some((uuid, revision, refs)) = &self.application {
            let current = store.registry(*uuid)?;
            ensure!(
                current.registry.revision == revision.load(Ordering::SeqCst)
                    && current.registry.policy_refs == *refs,
                Error::new(
                    ErrorCode::RevisionConflict,
                    "application policy associations changed"
                )
            );
        }
        Ok(())
    }
    /// Advance only after this operation successfully published its own app
    /// revision under the checked metadata lock. Concurrent writers never call it.
    pub(crate) fn published(&self, app: Uuid, revision: u64) {
        if let Some((expected_app, expected, _)) = &self.application
            && *expected_app == app
        {
            expected.store(revision, Ordering::SeqCst);
        }
    }
}

impl Store {
    pub(crate) fn with_policy_fence(&self, source: Uuid, app: Option<Uuid>) -> Result<Self> {
        let mut store = self.clone();
        store.policy_fence = Some(PolicyFence::capture(self, Some(source), app)?);
        Ok(store)
    }
    pub(crate) fn check_policy_action(
        &self,
        action: crate::models::claim::policy::Action,
    ) -> Result<()> {
        self.check_policy_fence()?;
        if let Some(fence) = &self.policy_fence {
            fence.authorize(action)?;
        }
        Ok(())
    }
    pub(crate) fn check_policy_fence(&self) -> Result<()> {
        if let Some(fence) = &self.policy_fence {
            fence.check(self)?;
        }
        Ok(())
    }
}
