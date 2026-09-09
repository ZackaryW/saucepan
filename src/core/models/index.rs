use super::{AppSettings, AppToken, Filters, Source};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppContext {
    pub version: u32,
    pub app: String,
    #[serde(default)]
    pub authoritative: bool,
    #[serde(default)]
    pub proof: Option<AppToken>,
    #[serde(default)]
    pub restrictions: Vec<String>,
}
impl AppContext {
    pub fn ordinary(app: impl Into<String>) -> Self {
        Self {
            version: super::FORMAT_VERSION,
            app: app.into(),
            authoritative: false,
            proof: None,
            restrictions: Vec::new(),
        }
    }
    pub fn authenticated(proof: AppToken) -> Self {
        Self {
            app: proof.app.clone(),
            proof: Some(proof),
            authoritative: true,
            ..Self::ordinary("")
        }
    }
}

/// Digests of consumed content; directory entries have no file digest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
    pub digest: Option<String>,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub id: String,
    pub source_id: String,
    pub source: Source,
    pub snapshot_id: String,
    pub revision: String,
    pub folder: Option<String>,
    pub content_id: String,
    pub files: BTreeMap<String, FileRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppView {
    pub version: u32,
    pub app: String,
    pub settings: AppSettings,
    pub filters: Filters,
    pub entries: BTreeMap<String, Artifact>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisteredApp {
    pub settings: AppSettings,
    pub filters: Filters,
    pub touched: BTreeSet<String>,
    pub token_binding: [u8; 32],
    pub data_hmac: [u8; 32],
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Index {
    pub version: u32,
    pub store_id: [u8; 32],
    pub apps: BTreeMap<String, RegisteredApp>,
    pub artifacts: BTreeMap<String, Artifact>,
    pub sources: BTreeMap<String, SourceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub id: String,
    pub revision: String,
    pub content_id: String,
    pub files: BTreeMap<String, FileRecord>,
    pub dependencies: BTreeMap<String, String>,
    pub zip_digest: Option<String>,
    pub last_used: u64,
    pub created: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceState {
    pub source: Source,
    pub current: Option<Snapshot>,
    pub history: Vec<Snapshot>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acquired {
    pub artifact: Artifact,
    pub directory: std::path::PathBuf,
    pub fallback: bool,
    pub update_checked: bool,
    pub content_verified: bool,
}
