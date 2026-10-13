//! Derived plans are persisted evidence, never accepted as caller policy authority.
use super::{
    Freshness, Identical, PayloadVerification, RevisionPolicy, UpdatePolicy,
    record::{AccessRules, Axis, CacheMode, Constraints},
};
use crate::models::{
    claim::{Scope, policy::ReclaimStrategy},
    store::{SelectionPolicy, VerificationMode},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct PolicyEvidence {
    pub uuid: Uuid,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Behavior {
    pub cache: CacheMode,
    pub freshness: Freshness,
    pub allow_stale: bool,
    pub payload_verification: PayloadVerification,
    pub path_verification: VerificationMode,
    pub selection: SelectionPolicy,
    pub identical: Identical,
    pub update: UpdatePolicy,
    pub revision: RevisionPolicy,
    pub reclaim: ReclaimStrategy,
    pub useable_app: bool,
}
impl Default for Behavior {
    fn default() -> Self {
        Self {
            cache: CacheMode::Cached,
            freshness: Freshness::default(),
            allow_stale: false,
            payload_verification: PayloadVerification::default(),
            path_verification: VerificationMode::default(),
            selection: SelectionPolicy::default(),
            identical: Identical::default(),
            update: UpdatePolicy::default(),
            revision: RevisionPolicy::default(),
            reclaim: ReclaimStrategy::default(),
            useable_app: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatedPolicies {
    pub behavior: Behavior,
    /// An absent axis comes from this contract's built-in default.
    pub evidence: BTreeMap<Axis, PolicyEvidence>,
    pub public_discovery: bool,
    pub public_scopes: BTreeSet<Scope>,
    pub access: Vec<AccessRules>,
    pub constraints: Vec<Constraints>,
}
