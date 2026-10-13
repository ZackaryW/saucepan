//! Serializable requests and evidence. Descriptors never confer a native lease.
use crate::models::{
    policy::{Freshness, PayloadVerification},
    source::{Resolved, Source},
    store::{AssetFilter, Platform, SelectionPolicy, Selector},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceInput {
    Inline { source: Source },
    Registered { source_uuid: Uuid },
}

/// Retrieval selects an original asset; folder/format/bin belong to acquisition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AssetSelection {
    pub platform: Option<Platform>,
    pub filters: Vec<AssetFilter>,
    pub selection_policy: SelectionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CachePolicy {
    Cached {
        #[serde(default)]
        freshness: Freshness,
        #[serde(default)]
        allow_stale: bool,
    },
    Temporary {},
    NoStore {
        destination: PathBuf,
    },
}
impl Default for CachePolicy {
    fn default() -> Self {
        Self::Cached {
            freshness: Freshness::default(),
            allow_stale: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrieveRequest {
    pub source: SourceInput,
    pub selector: Selector,
    #[serde(default)]
    pub asset: AssetSelection,
    #[serde(default)]
    pub cache: CachePolicy,
    #[serde(default)]
    pub verification: PayloadVerification,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaterialKind {
    File,
    Directory,
    GitRepository { bare: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Ownership {
    Managed { uuid: Uuid },
    Temporary {},
    Caller,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationEvidence {
    /// Present only after comparison against an expected digest.
    pub expected_sha256: Option<String>,
    /// Storage identity, independently of expected-digest verification.
    pub content_sha256: Option<String>,
    pub git_objects_validated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievedDescriptor {
    pub ownership: Ownership,
    pub path: PathBuf,
    pub kind: MaterialKind,
    pub source: SourceInput,
    pub resolved: Resolved,
    pub dependencies: BTreeMap<String, String>,
    pub verification: VerificationEvidence,
    pub checked_at: Option<u64>,
    pub reused: bool,
    pub stale: bool,
}
