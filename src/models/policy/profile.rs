//! Profiles are expansion results, never another persisted policy authority.
use super::{PolicyRecord, PolicyRefs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileName {
    #[default]
    Managed,
    ExplicitUpdate,
    Temporary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProfileRequest {
    pub name: ProfileName,
    pub for_install: bool,
    pub catalog: Vec<PolicyRecord>,
    pub retrieval_policy_refs: PolicyRefs,
    pub acquisition_policy_refs: PolicyRefs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileExpansion {
    pub name: ProfileName,
    pub for_install: bool,
    pub digest: String,
    pub catalog: Vec<PolicyRecord>,
    pub retrieval_policy_refs: PolicyRefs,
    pub acquisition_policy_refs: PolicyRefs,
    pub combined_policy_refs: PolicyRefs,
}
