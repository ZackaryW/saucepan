//! Narrow attachment observations avoid conflicts from unrelated catalog writes.
use crate::{
    core::Store,
    error::{Error, ErrorCode},
    models::policy::PolicyRefs,
};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PolicyFence {
    source: Option<(Uuid, u64, PolicyRefs)>,
    application: Option<(Uuid, u64, PolicyRefs)>,
    definitions: BTreeMap<Uuid, String>,
}
impl PolicyFence {
    pub fn capture(store: &Store, source: Option<Uuid>, app: Option<Uuid>) -> Result<Self> {
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
                        registry.registry.revision,
                        registry.registry.policy_refs,
                    ))
                })
                .transpose()?,
            definitions: BTreeMap::new(),
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
        Ok(fence)
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
                current.registry.revision == *revision && current.registry.policy_refs == *refs,
                Error::new(
                    ErrorCode::RevisionConflict,
                    "application policy associations changed"
                )
            );
        }
        Ok(())
    }
}
