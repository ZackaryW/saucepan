use super::policy::Refresh;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Local { path: PathBuf },
    Url { url: String },
    GithubRelease { repo: String },
    GithubRepo { repo: String },
    Git { origin: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    #[serde(default)]
    pub policy_refs: super::policy::PolicyRefs,
    #[serde(default)]
    pub policy_revision: u64,
    pub uuid: Uuid,
    pub source: Source,
    /// Explicit anonymous catalogue visibility; independent of claim visibility.
    #[serde(skip)]
    pub public_discovery: bool,
    #[serde(skip)]
    pub refresh: Refresh,
    /// Derived from the source's immutable policy definitions.
    #[serde(skip)]
    pub retrieval_freshness: Option<super::policy::Freshness>,
    #[serde(default)]
    pub discovery: BTreeMap<String, Discovery>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub checked_at: u64,
    pub resolved: Resolved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub url: String,
    pub digest: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Resolved {
    Local,
    Git {
        commit: String,
    },
    Release {
        id: u64,
        tag: String,
        assets: Vec<Asset>,
    },
    Url {
        url: String,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}
