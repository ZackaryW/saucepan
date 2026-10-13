//! Reconstruct adopted behavior from immutable definitions, then check it against
//! current authority. Serialized effective values alone can never grant access.
use super::{
    catalog::PolicyCatalog,
    resolve::{self, Inputs, Operation},
};
use crate::{
    error::{Error, ErrorCode},
    models::{
        claim::adopted::AdoptedPolicies,
        policy::{PolicyDefinition, PolicyRefs, effective::EvaluatedPolicies, record::Axis},
    },
};
use anyhow::{Result, ensure};

impl crate::core::Store {
    /// Adopted choices are resolved against current restrictions, never merged
    /// with current defaults a second time by the execution engine.
    pub(crate) fn claim_execution_policy(
        &self,
        source: uuid::Uuid,
        registry: &crate::models::claim::Registry,
        adopted: Option<&AdoptedPolicies>,
    ) -> Result<crate::models::claim::policy::Effective> {
        let snapshot = self.metadata.snapshot()?;
        let catalog = super::persistence::load(&self.metadata, &snapshot)?;
        let source = self.source(source)?;
        let adopted = adopted.ok_or_else(|| {
            Error::new(
                ErrorCode::IntegrityError,
                "claim is missing adopted policy evidence",
            )
        })?;
        let plan = surviving(
            &catalog,
            adopted,
            &source.policy_refs,
            &registry.policy_refs,
        )?;
        super::adopt::effective(&plan)
    }

    pub(crate) fn acquisition_execution_policy(
        &self,
        source: uuid::Uuid,
        registry: &crate::models::claim::Registry,
        acquisition: &crate::models::store::AcquisitionRequest,
    ) -> Result<crate::models::claim::policy::Effective> {
        if let Some(adopted) = acquisition.adopted.as_deref() {
            return self.claim_execution_policy(source, registry, Some(adopted));
        }
        let snapshot = self.metadata.snapshot()?;
        let catalog = super::persistence::load(&self.metadata, &snapshot)?;
        let source = self.source(source)?;
        super::compiled::evaluate(
            &catalog,
            Inputs {
                source: &source.policy_refs,
                application: &registry.policy_refs,
                request: &PolicyRefs::default(),
                pinned: None,
                operation: Operation::Acquire,
                managed_output: matches!(
                    acquisition.destination,
                    crate::models::store::Destination::Managed {}
                ),
                update_due: false,
            },
            &crate::models::claim::policy::Overrides {
                identical: Some(acquisition.identical),
                revision: super::composition::effective_revision(acquisition)?,
                useable_app: Some(acquisition.protection.is_some()),
                update_policy: Some(acquisition.update_policy),
                ..Default::default()
            },
        )
    }
}

pub(crate) fn validate_existing(
    catalog: &PolicyCatalog,
    claim: &crate::models::claim::Claim,
    source: &PolicyRefs,
    application: &PolicyRefs,
) -> Result<EvaluatedPolicies> {
    let plan = match claim.request.adopted.as_deref() {
        Some(adopted) => surviving(catalog, adopted, source, application)?,
        None => {
            return Err(Error::new(
                ErrorCode::IntegrityError,
                "claim is missing adopted policy evidence",
            )
            .into());
        }
    };
    ensure!(
        super::adopt::effective(&plan)? == claim.effective,
        Error::new(
            ErrorCode::PolicyConflict,
            "claim execution differs from adopted policy"
        )
    );
    Ok(plan)
}

pub(crate) fn validate_adopted(catalog: &PolicyCatalog, adopted: &AdoptedPolicies) -> Result<()> {
    let mut reconstructed = EvaluatedPolicies::default();
    for (axis, expected) in &adopted.evidence {
        let record = catalog.get(expected.uuid)?;
        ensure!(
            resolve::evidence(record)? == *expected,
            Error::new(
                ErrorCode::IntegrityError,
                "adopted definition digest mismatch"
            )
        );
        if *axis == Axis::Protection
            && matches!(&record.definition,
            PolicyDefinition::PolicyConstraints(bounds) if bounds.require_useable_app)
        {
            reconstructed.behavior.useable_app = true;
            reconstructed.evidence.insert(*axis, expected.clone());
        } else {
            ensure!(
                record.definition.axis() == Some(*axis),
                Error::new(
                    ErrorCode::IntegrityError,
                    "adopted definition target mismatch"
                )
            );
            resolve::apply(&mut reconstructed, record)?;
        }
    }
    ensure!(
        reconstructed.behavior == adopted.behavior,
        Error::new(
            ErrorCode::IntegrityError,
            "adopted behavior does not match definitions and built-ins"
        )
    );
    resolve::filters(catalog, &adopted.filter_refs)?;
    Ok(())
}

pub(crate) fn surviving(
    catalog: &PolicyCatalog,
    adopted: &AdoptedPolicies,
    source: &PolicyRefs,
    application: &PolicyRefs,
) -> Result<EvaluatedPolicies> {
    validate_adopted(catalog, adopted)?;
    let pinned = adopted.pinned();
    let empty = PolicyRefs::default();
    let current = resolve::evaluate(
        catalog,
        Inputs {
            source,
            application,
            request: &empty,
            pinned: Some(&pinned),
            operation: Operation::Compose,
            managed_output: true,
            update_due: false,
        },
    )?;
    ensure!(
        current.behavior == adopted.behavior,
        Error::new(
            ErrorCode::PolicyConflict,
            "current constraints would reinterpret an adopted claim"
        )
    );
    Ok(current)
}

pub(crate) fn behavior_refs(
    catalog: &PolicyCatalog,
    adopted: &AdoptedPolicies,
) -> Result<PolicyRefs> {
    adopted
        .evidence
        .values()
        .filter_map(|evidence| match catalog.get(evidence.uuid) {
            Ok(record) if record.definition.axis().is_some() => Some(Ok(evidence.uuid)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<crate::error::Result<Vec<_>>>()?
        .try_into()
        .map_err(Into::into)
}
