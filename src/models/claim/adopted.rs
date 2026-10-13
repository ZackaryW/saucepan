//! Checked evidence of adopted choices; this is never accepted as request authority.
use crate::models::policy::{
    PolicyRefs,
    effective::{Behavior, EvaluatedPolicies, PolicyEvidence},
    record::Axis,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptedPolicies {
    pub behavior: Behavior,
    pub evidence: BTreeMap<Axis, PolicyEvidence>,
    pub filter_refs: PolicyRefs,
}
impl AdoptedPolicies {
    pub fn from_plan(plan: &EvaluatedPolicies, filter_refs: PolicyRefs) -> Self {
        Self {
            behavior: plan.behavior.clone(),
            evidence: plan
                .evidence
                .iter()
                .filter(|(axis, _)| !matches!(axis, Axis::SourceVisibility | Axis::ClaimVisibility))
                .map(|(axis, evidence)| (*axis, evidence.clone()))
                .collect(),
            filter_refs,
        }
    }
    pub(crate) fn pinned(&self) -> EvaluatedPolicies {
        EvaluatedPolicies {
            behavior: self.behavior.clone(),
            evidence: self.evidence.clone(),
            ..Default::default()
        }
    }
}
