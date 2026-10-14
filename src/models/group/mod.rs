//! Saved resource intent and exact selected generations, independent of live guards.
mod request;
use crate::models::{claim::Scope, policy::effective::PolicyEvidence};
pub use request::{AcquireMember, ClaimIntent, GroupDefinition, Member, Target};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupRecord {
    pub uuid: Uuid,
    pub owner: Option<Uuid>,
    pub revision: u64,
    pub definition: GroupDefinition,
    pub witnesses: BTreeMap<String, Witness>,
    pub selected: Option<Generation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Witness {
    pub source_uuid: Uuid,
    pub scope: Option<Scope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    pub uuid: Uuid,
    pub members: BTreeMap<String, SelectedMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedMember {
    pub output_uuid: Uuid,
    pub claim_uuid: Option<Uuid>,
    pub scope: Option<Scope>,
    pub relative: Option<String>,
    pub policy_provenance: Vec<PolicyEvidence>,
    pub selected_source: crate::models::source::Resolved,
    pub checked_source: Option<crate::models::source::Resolved>,
    pub last_success: Option<u64>,
}
