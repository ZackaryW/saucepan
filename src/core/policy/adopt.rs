//! Revision-checked associations. Existing execution structs are populated from
//! the evaluated definitions during the staged public-contract cutover.
use super::{
    persistence,
    resolve::{self, Inputs, Operation},
};
use crate::{
    core::{Store, index::Journal},
    error::{Error, ErrorCode},
    models::{
        claim::{
            policy::{AccessPolicy, Effective, Overrides, ReclaimPolicy, Registration},
            security::Credential,
        },
        index::Record,
        policy::{Policies, PolicyRefs, Protection, effective::EvaluatedPolicies},
        source::SourceRecord,
    },
};
use anyhow::{Result, ensure};
use std::collections::BTreeSet;
use uuid::Uuid;

fn intersection<T: Clone + Ord>(left: &mut Option<BTreeSet<T>>, right: &Option<BTreeSet<T>>) {
    if let Some(right) = right {
        *left = Some(match left {
            Some(left) => left.intersection(right).cloned().collect(),
            None => right.clone(),
        });
    }
}
pub(crate) fn access(plan: &EvaluatedPolicies) -> AccessPolicy {
    let mut access = AccessPolicy {
        public_scopes: plan.public_scopes.clone(),
        ..Default::default()
    };
    for rules in &plan.access {
        intersection(&mut access.sources, &rules.sources);
        intersection(&mut access.scopes, &rules.scopes);
        intersection(&mut access.actions, &rules.actions);
    }
    access
}
pub(crate) fn effective(plan: &EvaluatedPolicies) -> Result<Effective> {
    let b = &plan.behavior;
    Ok(Effective {
        output: Policies {
            identical: b.identical,
            eviction: b.revision.try_into()?,
            protection: b.useable_app.then_some(Protection::UseableApp),
        },
        reclaim: ReclaimPolicy::ReclaimPolicy {
            strategy: b.reclaim,
        },
        update_policy: b.update,
    })
}
pub(crate) fn pinned(effective: &Effective) -> Overrides {
    let crate::models::policy::Eviction::Latest { keep } = effective.output.eviction;
    Overrides {
        identical: Some(effective.output.identical),
        keep: Some(keep),
        reclaim: Some(effective.reclaim.clone()),
        useable_app: Some(effective.output.protection.is_some()),
        update_policy: Some(effective.update_policy),
    }
}
fn survivor(effective: &Effective) -> Result<EvaluatedPolicies> {
    let mut plan = EvaluatedPolicies::default();
    plan.behavior.identical = effective.output.identical;
    plan.behavior.revision = (&effective.output.eviction).try_into()?;
    plan.behavior.update = effective.update_policy;
    plan.behavior.reclaim = effective.reclaim.strategy();
    plan.behavior.useable_app = effective.output.protection.is_some();
    Ok(plan)
}
pub(crate) fn registration(plan: &EvaluatedPolicies) -> Result<Registration> {
    let mut registration = Registration {
        defaults: effective(plan)?,
        ..Default::default()
    };
    for bound in &plan.constraints {
        registration.minimum_keep = registration.minimum_keep.max(bound.minimum_keep);
        registration.maximum_keep = registration.maximum_keep.min(bound.maximum_keep);
        registration
            .allowed_identical
            .retain(|value| bound.allowed_identical.contains(value));
        registration
            .allowed_reclaim
            .retain(|value| bound.allowed_reclaim.contains(value));
        registration.allow_useable_app &= bound.allow_useable_app;
        registration.allow_automatic_updates &= bound.allow_automatic_updates;
        registration.minimum_update_interval = registration
            .minimum_update_interval
            .max(bound.minimum_update_interval);
    }
    registration.validate()?;
    Ok(registration)
}

pub(crate) fn application_settings(
    catalog: &super::catalog::PolicyCatalog,
    refs: &PolicyRefs,
) -> Result<(AccessPolicy, Registration)> {
    let empty = PolicyRefs::default();
    let plan = resolve::evaluate(
        catalog,
        Inputs {
            source: &empty,
            application: refs,
            request: &empty,
            pinned: None,
            operation: Operation::Configure,
            managed_output: true,
            update_due: false,
        },
    )?;
    Ok((access(&plan), registration(&plan)?))
}

impl Store {
    pub fn adopt_claim_policies(
        &self,
        app: Uuid,
        claim_id: Uuid,
        expected_revision: u64,
        refs: PolicyRefs,
    ) -> Result<()> {
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        let claim = registry
            .claims
            .get(&claim_id)
            .ok_or_else(|| Error::new(ErrorCode::UnknownClaim, "claim not registered"))?;
        self.authorize_resource(
            &registry,
            crate::models::claim::policy::Action::Update,
            Some(claim.request.source_uuid),
            Some(&claim.scope),
        )?;
        ensure!(
            application.revision == expected_revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "claim policy revision conflict"
            )
        );
        let source = self.source(claim.request.source_uuid)?;
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let plan = resolve::evaluate(
            &catalog,
            Inputs {
                source: &source.policy_refs,
                application: &registry.registry.policy_refs,
                request: &refs,
                pinned: None,
                operation: Operation::Compose,
                managed_output: true,
                update_due: false,
            },
        )?;
        let selected = effective(&plan)?;
        ensure!(
            plan.behavior.cache == crate::models::policy::record::CacheMode::Cached,
            Error::new(
                ErrorCode::PolicyConflict,
                "persistent claims require cached retrieval"
            )
        );
        let adopted = crate::models::claim::adopted::AdoptedPolicies::from_plan(
            &plan,
            claim
                .request
                .adopted
                .as_ref()
                .map(|p| p.filter_refs.clone())
                .unwrap_or_default(),
        );
        let selected_refs = super::claims::behavior_refs(&catalog, &adopted)?;
        let overrides = pinned(&selected);
        registry.registry.policies.evaluate(&overrides)?;
        if claim.policy_refs == selected_refs
            && claim.request.adopted.as_deref() == Some(&adopted)
            && claim.effective == selected
        {
            return Ok(());
        }
        let claim = registry.claims.get_mut(&claim_id).expect("checked claim");
        claim.policy_refs = selected_refs;
        claim.request.adopted = Some(Box::new(adopted));
        claim.effective = selected;
        claim.overrides = overrides;
        claim.request.policies = claim.effective.output.clone();
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            registries: vec![registry],
            ..Default::default()
        })
    }

    pub fn adopt_source_policies(
        &self,
        id: Uuid,
        expected_revision: u64,
        refs: PolicyRefs,
    ) -> Result<SourceRecord> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "source policy adoption requires administration"
            )
        );
        let _source = crate::utils::lock::LockGuard::acquire(
            &self.root.join(format!(".locks/source-{id}")),
            crate::utils::lock::LockMode::Exclusive,
            std::time::Duration::from_secs(150),
        )?;
        let _lock = self.lock()?;
        let mut source = self.source(id)?;
        ensure!(
            source.policy_revision == expected_revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "source policy revision conflict"
            )
        );
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let empty = PolicyRefs::default();
        let plan = resolve::evaluate(
            &catalog,
            Inputs {
                source: &refs,
                application: &empty,
                request: &empty,
                pinned: None,
                operation: Operation::Configure,
                managed_output: true,
                update_due: false,
            },
        )?;
        if source.policy_refs == refs {
            return Ok(source);
        }
        for reference in snapshot
            .catalog
            .records
            .values()
            .filter(|r| r.identity.kind == crate::core::claim::security::Kind::Registry)
        {
            let app = reference
                .identity
                .owner
                .ok_or_else(crate::core::claim::security::integrity_error)?;
            let registry = self.registry(app)?;
            for claim in registry
                .claims
                .values()
                .filter(|claim| claim.request.source_uuid == id)
            {
                if let Some(adopted) = &claim.request.adopted {
                    super::claims::surviving(
                        &catalog,
                        adopted,
                        &refs,
                        &registry.registry.policy_refs,
                    )?;
                    continue;
                }
                let pinned = survivor(&claim.effective)?;
                let next = resolve::evaluate(
                    &catalog,
                    Inputs {
                        source: &refs,
                        application: &registry.registry.policy_refs,
                        request: &empty,
                        pinned: Some(&pinned),
                        operation: Operation::Compose,
                        managed_output: true,
                        update_due: false,
                    },
                )?;
                ensure!(
                    effective(&next)? == claim.effective,
                    Error::new(
                        ErrorCode::PolicyConflict,
                        "source constraints would reinterpret a surviving claim"
                    )
                );
            }
        }
        source.policy_refs = refs;
        source.policy_revision = source
            .policy_revision
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::RevisionConflict, "source revision exhausted"))?;
        source.public_discovery = plan.public_discovery;
        source.retrieval_freshness = Some(plan.behavior.freshness);
        if let crate::models::policy::Freshness::MaxAge { seconds } = plan.behavior.freshness {
            source.refresh.ttl_seconds = seconds;
        }
        self.save_source(&source)?;
        Ok(source)
    }

    pub fn adopt_application_policies(
        &self,
        app: Uuid,
        expected_revision: u64,
        refs: PolicyRefs,
        proof: Option<&Credential>,
    ) -> Result<()> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "application policy adoption requires administration"
            )
        );
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        self.metadata.authenticator().check_credential(
            app,
            &registry.registry.credential,
            proof,
        )?;
        ensure!(
            application.revision == expected_revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "application policy revision conflict"
            )
        );
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let empty = PolicyRefs::default();
        let plan = resolve::evaluate(
            &catalog,
            Inputs {
                source: &empty,
                application: &refs,
                request: &empty,
                pinned: None,
                operation: Operation::Configure,
                managed_output: true,
                update_due: false,
            },
        )?;
        let settings = registration(&plan)?;
        // Pin surviving choices before changing defaults. Registry validation
        // rejects tightened constraints that would invalidate any survivor.
        for claim in registry.claims.values_mut() {
            if let Some(adopted) = &claim.request.adopted {
                super::claims::surviving(
                    &catalog,
                    adopted,
                    &self.source(claim.request.source_uuid)?.policy_refs,
                    &refs,
                )?;
            }
            claim.overrides = pinned(&claim.effective);
            settings.evaluate(&claim.overrides)?;
        }
        if registry.registry.policy_refs == refs {
            return Ok(());
        }
        registry.registry.policy_refs = refs;
        registry.registry.access = access(&plan);
        registry.registry.policies = settings;
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            registries: vec![registry],
            ..Default::default()
        })
    }
}
