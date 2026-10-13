//! Source convenience changes still publish immutable definitions and references.
use super::{
    catalog::PolicyCatalog,
    persistence,
    resolve::{self, Inputs, Operation},
};
use crate::{
    core::{
        Store,
        claim::security::{Identity, Kind},
    },
    error::{Error, ErrorCode},
    models::{
        policy::{PolicyRecord, PolicyRefs, record::PolicyDefinition},
        source::SourceRecord,
    },
};
use anyhow::{Result, ensure};
use uuid::Uuid;

pub(crate) fn hydrate(source: &mut SourceRecord, catalog: &PolicyCatalog) -> Result<()> {
    let empty = PolicyRefs::default();
    let plan = resolve::evaluate(
        catalog,
        Inputs {
            source: &source.policy_refs,
            application: &empty,
            request: &empty,
            pinned: None,
            operation: Operation::Configure,
            managed_output: true,
            update_due: false,
        },
    )?;
    source.public_discovery = plan.public_discovery;
    source.retrieval_freshness = plan
        .evidence
        .contains_key(&crate::models::policy::record::Axis::Refresh)
        .then_some(plan.behavior.freshness);
    source.refresh = Default::default();
    if let crate::models::policy::Freshness::MaxAge { seconds } = plan.behavior.freshness {
        source.refresh.ttl_seconds = seconds;
    }
    Ok(())
}

impl Store {
    pub(crate) fn source_behavior(
        &self,
        id: Uuid,
        definition: PolicyDefinition,
    ) -> Result<SourceRecord> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "source configuration requires administration"
            )
        );
        let _source = crate::utils::lock::LockGuard::acquire(
            &self.root.join(format!(".locks/source-{id}")),
            crate::utils::lock::LockMode::Exclusive,
            std::time::Duration::from_secs(150),
        )?;
        let _lock = self.lock()?;
        let mut source = self.source(id)?;
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let same_axis = |other: &PolicyDefinition| {
            std::mem::discriminant(other) == std::mem::discriminant(&definition)
        };
        let mut refs = Vec::new();
        for reference in source.policy_refs.ids() {
            let record = catalog.get(*reference)?;
            if record.definition == definition {
                return Ok(source);
            }
            if !same_axis(&record.definition) {
                refs.push(*reference);
            }
        }
        let record = PolicyRecord {
            uuid: Uuid::new_v4(),
            definition,
        };
        refs.push(record.uuid);
        source.policy_refs = refs.try_into()?;
        source.policy_revision = source
            .policy_revision
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::RevisionConflict, "source revision exhausted"))?;
        let combined = PolicyCatalog::new(catalog.records().cloned().chain([record.clone()]))?;
        hydrate(&mut source, &combined)?;
        self.metadata.publish(
            &snapshot,
            vec![
                (
                    persistence::identity(record.uuid),
                    serde_json::to_value(record)?,
                ),
                (
                    Identity {
                        kind: Kind::Source,
                        owner: None,
                        member: Some(id),
                    },
                    serde_json::to_value(&source)?,
                ),
            ],
            vec![],
        )?;
        Ok(source)
    }
}
