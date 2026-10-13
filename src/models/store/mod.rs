pub mod acquisition;
use super::{policy::Policies, source::Resolved};
pub use acquisition::AcquisitionState;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use uuid::Uuid;
mod composition;
pub use composition::{AcquiredDescriptor, AcquisitionRequest, Destination};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selector {
    Local,
    LatestStable,
    Tag { tag: String },
    Ref { reference: String },
    Commit { commit: String },
    Url,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub os: String,
    pub arch: String,
}
impl Platform {
    pub fn host() -> Self {
        Self {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetFilter {
    pub platform: Option<Platform>,
    pub asset_patterns: Vec<String>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    #[default]
    Natural,
    Raw,
    Zip,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyTarget {
    GithubReleaseAssetFilter,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    FirstMatchWin,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionPolicy {
    pub policy_target: PolicyTarget,
    pub strategy: Strategy,
}
impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            policy_target: PolicyTarget::GithubReleaseAssetFilter,
            strategy: Strategy::FirstMatchWin,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub platform: Option<Platform>,
    pub filters: Vec<AssetFilter>,
    pub selection_policy: SelectionPolicy,
    pub format: Format,
    pub filename: Option<String>,
    pub folder: Option<String>,
    pub bin: Vec<String>,
    pub sha256: Option<String>,
}
/// Reference-only intent for a managed output. Policies must be registered first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub source_uuid: Uuid,
    pub selector: Selector,
    #[serde(default)]
    pub selection: crate::models::claim::request::Selection,
    #[serde(default)]
    pub policy_refs: crate::models::policy::PolicyRefs,
}

/// Frozen execution evidence written by the policy planner, never accepted as input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputPlan {
    #[serde(default)]
    pub adopted: Option<Box<crate::models::claim::adopted::AdoptedPolicies>>,
    pub source_uuid: Uuid,
    pub selector: Selector,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub policies: Policies,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Ready,
    ReclaimPending,
    Reclaimed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub digest: Option<String>,
    pub executable: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputEntry {
    pub uuid: Uuid,
    pub request: OutputPlan,
    pub item_key: String,
    pub fingerprint: String,
    pub resolved: Resolved,
    pub content_digest: String,
    pub files: BTreeMap<String, FileEntry>,
    pub dependencies: BTreeMap<String, String>,
    pub sequence: u64,
    pub state: Availability,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acquired {
    pub entry: OutputEntry,
    pub directory: PathBuf,
    pub reused: bool,
    pub update_checked: bool,
    pub stale: bool,
    pub reclaim: ReclaimReport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReclaimReport {
    pub effective_keep: BTreeMap<String, usize>,
    pub reclaimed: Vec<Uuid>,
    pub deferred: BTreeMap<Uuid, String>,
    pub pending: BTreeMap<Uuid, String>,
}
/// Per-call checks; never persisted as acquisition identity or freshness.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    #[default]
    Metadata,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedResource {
    pub entry_uuid: uuid::Uuid,
    pub directory: std::path::PathBuf,
    pub verification: VerificationMode,
}
