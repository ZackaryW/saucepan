//! Pure policy planning. Session ownership/proof and publication fences stay at
//! their existing boundaries; a request cannot manufacture those inputs.
use super::catalog::PolicyCatalog;
use crate::{
    error::{Error, ErrorCode, Result},
    models::{
        claim::{Scope, policy::Action},
        policy::{
            Freshness, PolicyDefinition as Def, PolicyRecord, PolicyRefs, UpdatePolicy,
            effective::{EvaluatedPolicies, PolicyEvidence},
            record::{Axis, CacheMode, VerificationPolicy},
        },
        store::AssetFilter,
    },
    utils::tree::digest,
};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Retrieve,
    Acquire,
    Compose,
    Read,
    Configure,
}
impl Operation {
    fn accepts(self, axis: Axis) -> bool {
        use Axis::*;
        match axis {
            Cache | Refresh | Stale | PayloadVerification | Selection => {
                matches!(self, Self::Retrieve | Self::Compose | Self::Configure)
            }
            Identical | Update | Revision | Reclaim | Protection => {
                matches!(self, Self::Acquire | Self::Compose | Self::Configure)
            }
            PathVerification => matches!(self, Self::Read | Self::Configure),
            SourceVisibility | ClaimVisibility => self == Self::Configure,
        }
    }
}

pub struct Inputs<'a> {
    pub source: &'a PolicyRefs,
    pub application: &'a PolicyRefs,
    pub request: &'a PolicyRefs,
    /// Internal derived claim state, not caller JSON.
    pub pinned: Option<&'a EvaluatedPolicies>,
    pub operation: Operation,
    pub managed_output: bool,
    pub update_due: bool,
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
fn conflict(message: &str) -> Error {
    Error::new(ErrorCode::PolicyConflict, message)
}

fn evidence(record: &PolicyRecord) -> Result<PolicyEvidence> {
    let bytes =
        serde_json::to_vec(record).map_err(|_| invalid("cannot encode policy definition"))?;
    Ok(PolicyEvidence {
        uuid: record.uuid,
        digest: digest(bytes.as_slice()).map_err(Error::from)?,
    })
}

fn apply(plan: &mut EvaluatedPolicies, record: &PolicyRecord) -> Result<()> {
    let b = &mut plan.behavior;
    match &record.definition {
        Def::CachePolicy { mode } => b.cache = *mode,
        Def::RefreshPolicy(value) => b.freshness = *value,
        Def::StalePolicy { allow_stale } => b.allow_stale = *allow_stale,
        Def::VerificationPolicy(VerificationPolicy::Payload { mode }) => {
            b.payload_verification = *mode
        }
        Def::VerificationPolicy(VerificationPolicy::Path { mode }) => b.path_verification = *mode,
        Def::SelectionPolicy(value) => b.selection = value.clone(),
        Def::IdenticalPullPolicy { strategy } => b.identical = *strategy,
        Def::UpdatePolicy(value) => b.update = *value,
        Def::RevisionPolicy(value) => b.revision = *value,
        Def::ReclaimPolicy { strategy } => b.reclaim = *strategy,
        Def::UseableAppPolicy {} => b.useable_app = true,
        Def::SourceVisibilityPolicy { public_discovery } => {
            plan.public_discovery = *public_discovery
        }
        Def::ClaimVisibilityPolicy { public_scopes } => plan.public_scopes = public_scopes.clone(),
        _ => return Err(invalid("expected a singleton behavior policy")),
    }
    if let Some(axis) = record.definition.axis() {
        plan.evidence.insert(axis, evidence(record)?);
    }
    Ok(())
}

pub fn evaluate(catalog: &PolicyCatalog, input: Inputs<'_>) -> Result<EvaluatedPolicies> {
    let mut plan = EvaluatedPolicies::default();
    if let Some(pinned) = input.pinned {
        plan.behavior = pinned.behavior.clone();
        plan.evidence = pinned.evidence.clone();
        plan.evidence.remove(&Axis::SourceVisibility);
        plan.evidence.remove(&Axis::ClaimVisibility);
    }
    let mut required_protection = None;
    for (level, refs) in [input.source, input.application, input.request]
        .into_iter()
        .enumerate()
    {
        let mut axes = BTreeSet::new();
        for id in refs.ids() {
            let record = catalog.get(*id)?;
            let definition = &record.definition;
            definition.validate()?;
            if let Some(axis) = definition.axis()
                && !axes.insert(axis)
            {
                return Err(conflict(
                    "multiple singleton policies at one precedence level",
                ));
            }
            match definition {
                Def::AccessPolicy(rules) if level != 2 => plan.access.push(rules.clone()),
                Def::PolicyConstraints(bounds) if level != 2 => {
                    plan.constraints.push(bounds.clone());
                    if bounds.require_useable_app && input.managed_output {
                        required_protection = Some(evidence(record)?);
                    }
                }
                Def::SourceVisibilityPolicy { .. } if level == 0 => apply(&mut plan, record)?,
                Def::ClaimVisibilityPolicy { .. } if level == 1 => apply(&mut plan, record)?,
                Def::AccessPolicy(_)
                | Def::PolicyConstraints(_)
                | Def::SourceVisibilityPolicy { .. }
                | Def::ClaimVisibilityPolicy { .. }
                | Def::GithubReleaseAssetFilter(_) => {
                    return Err(invalid("policy is not applicable to this association"));
                }
                _ => {
                    let axis = definition
                        .axis()
                        .ok_or_else(|| invalid("missing policy axis"))?;
                    if level == 2 && !input.operation.accepts(axis) {
                        return Err(invalid("policy targets another operation"));
                    }
                    if level != 2
                        && !input.managed_output
                        && matches!(axis, Axis::Revision | Axis::Protection)
                    {
                        continue;
                    }
                    if input.operation.accepts(axis) && (input.pinned.is_none() || level == 2) {
                        apply(&mut plan, record)?;
                    }
                }
            }
        }
    }
    if let Some(required) = required_protection {
        plan.behavior.useable_app = true;
        plan.evidence.entry(Axis::Protection).or_insert(required);
    }
    validate(&plan, &input)?;
    Ok(plan)
}

fn validate(plan: &EvaluatedPolicies, input: &Inputs<'_>) -> Result<()> {
    let b = &plan.behavior;
    if matches!(input.operation, Operation::Retrieve | Operation::Compose) {
        if b.cache != CacheMode::Cached
            && (b.allow_stale || matches!(b.freshness, Freshness::CacheOnly {}))
        {
            return Err(conflict(
                "fresh retrieval conflicts with cached-only behavior",
            ));
        }
        if input.update_due && matches!(b.freshness, Freshness::CacheOnly {}) {
            return Err(conflict(
                "due acquisition update conflicts with cache-only retrieval",
            ));
        }
    }
    if matches!(input.operation, Operation::Acquire | Operation::Compose) {
        if !input.managed_output && (plan.evidence.contains_key(&Axis::Revision) || b.useable_app) {
            return Err(conflict(
                "managed revision or protection policy requires managed output",
            ));
        }
        let keep = b.revision.keep()?;
        for bounds in &plan.constraints {
            if (input.managed_output
                && !(bounds.minimum_keep..=bounds.maximum_keep).contains(&keep))
                || !bounds.allowed_identical.contains(&b.identical)
                || !bounds.allowed_reclaim.contains(&b.reclaim)
                || (b.useable_app && !bounds.allow_useable_app)
            {
                return Err(conflict("behavior falls outside registered constraints"));
            }
            let interval = match b.update {
                UpdatePolicy::Manual {} => None,
                UpdatePolicy::Always {} => Some(0),
                UpdatePolicy::MaxAge { seconds } => Some(seconds),
            };
            if interval.is_some_and(|seconds| {
                !bounds.allow_automatic_updates || seconds < bounds.minimum_update_interval
            }) {
                return Err(conflict(
                    "update behavior falls outside registered constraints",
                ));
            }
        }
    }
    Ok(())
}

/// Current restrictions only. Identity, ownership and credentials are checked by
/// the calling session before this pure intersection of resource dimensions.
pub fn authorize(
    plan: &EvaluatedPolicies,
    action: Action,
    source: Option<Uuid>,
    scope: Option<&Scope>,
) -> Result<()> {
    for rules in &plan.access {
        if rules
            .actions
            .as_ref()
            .is_some_and(|set| !set.contains(&action))
            || rules
                .sources
                .as_ref()
                .is_some_and(|set| !source.is_some_and(|id| set.contains(&id)))
            || rules
                .scopes
                .as_ref()
                .is_some_and(|set| !scope.is_some_and(|value| set.contains(value)))
        {
            return Err(Error::new(
                ErrorCode::AccessDenied,
                "operation is outside registered policy allowances",
            ));
        }
    }
    Ok(())
}

pub fn filters(catalog: &PolicyCatalog, refs: &PolicyRefs) -> Result<Vec<AssetFilter>> {
    refs.ids()
        .iter()
        .map(|id| match &catalog.get(*id)?.definition {
            Def::GithubReleaseAssetFilter(filter) => Ok(filter.clone()),
            _ => Err(invalid("filter reference is not a release asset filter")),
        })
        .collect()
}
