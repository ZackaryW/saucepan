//! Post-retrieval selections cannot change provider or asset selection.
use super::Format;
use crate::models::{
    policy::{Identical, Protection, RevisionPolicy, UpdatePolicy},
    retrieval::{Ownership, VerificationEvidence},
    source::Resolved,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Destination {
    Managed {},
    Temporary {},
    Directory { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AcquisitionRequest {
    pub provenance: Vec<crate::models::policy::effective::PolicyEvidence>,
    pub destination: Destination,
    pub format: Format,
    pub filename: Option<String>,
    pub folder: Option<String>,
    pub bin: Vec<String>,
    pub identical: Identical,
    pub update_policy: UpdatePolicy,
    /// Absence means latest ten for managed outputs, no policy for other outputs.
    pub revision_policy: Option<RevisionPolicy>,
    pub protection: Option<Protection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquiredDescriptor {
    pub ownership: Ownership,
    pub path: PathBuf,
    pub selected_source: Resolved,
    pub checked_source: Option<Resolved>,
    pub verification: VerificationEvidence,
    pub reused: bool,
    pub changed: bool,
    pub stale: bool,
    pub update_policy: UpdatePolicy,
    pub revision_policy: Option<RevisionPolicy>,
    pub reclaim: Option<super::ReclaimReport>,
}

impl Default for Destination {
    fn default() -> Self {
        Self::Managed {}
    }
}
