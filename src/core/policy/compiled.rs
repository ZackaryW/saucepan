//! Normalize the remaining compiled native inputs into the shared resolver.
//! This module is private and does not define another policy evaluation model.
use super::{
    catalog::PolicyCatalog,
    resolve::{self, Inputs, Operation},
};
use crate::{
    error::{Error, ErrorCode},
    models::{
        claim::policy::{Effective, Overrides},
        policy::{PolicyDefinition as Def, PolicyRecord, PolicyRefs, effective::EvaluatedPolicies},
    },
};
use anyhow::{Result, ensure};

pub(crate) fn evaluate(
    catalog: &PolicyCatalog,
    input: Inputs<'_>,
    values: &Overrides,
) -> Result<Effective> {
    let definitions = [
        values
            .identical
            .map(|strategy| Def::IdenticalPullPolicy { strategy }),
        values.revision.map(Def::RevisionPolicy),
        values.reclaim.as_ref().map(|policy| Def::ReclaimPolicy {
            strategy: policy.strategy(),
        }),
        values.update_policy.map(Def::UpdatePolicy),
        (values.useable_app == Some(true)).then_some(Def::UseableAppPolicy {}),
    ]
    .into_iter()
    .flatten()
    .map(|definition| PolicyRecord {
        uuid: uuid::Uuid::new_v4(),
        definition,
    })
    .collect::<Vec<_>>();
    let plan = resolve::evaluate_compiled(catalog, input, &definitions)?;
    ensure!(
        values.useable_app != Some(false) || !plan.behavior.useable_app,
        Error::new(
            ErrorCode::PolicyConflict,
            "compiled input cannot disable attached protection"
        )
    );
    super::adopt::effective(&plan)
}

pub(crate) fn surviving(
    catalog: &PolicyCatalog,
    effective: &Effective,
    source: &PolicyRefs,
    application: &PolicyRefs,
) -> Result<EvaluatedPolicies> {
    let mut pinned = EvaluatedPolicies::default();
    pinned.behavior.identical = effective.output.identical;
    pinned.behavior.revision = effective.output.revision;
    pinned.behavior.update = effective.update_policy;
    pinned.behavior.reclaim = effective.reclaim.strategy();
    pinned.behavior.useable_app = effective.output.protection.is_some();
    let empty = PolicyRefs::default();
    let plan = resolve::evaluate(
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
        super::adopt::effective(&plan)? == *effective,
        Error::new(
            ErrorCode::PolicyConflict,
            "current constraints would reinterpret a surviving claim"
        )
    );
    Ok(plan)
}
