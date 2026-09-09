mod index;
mod recipe;
pub use index::{Acquired, AppContext, AppView, Artifact, FileRecord, Snapshot, SourceState};
pub(crate) use index::{Index, RegisteredApp};
pub use recipe::{Download, Recipe, Source};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub retain_snapshots: bool,
    pub verify_content: bool,
    pub allow_local_fallback: bool,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            retain_snapshots: true,
            verify_content: false,
            allow_local_fallback: false,
        }
    }
}

/// Selection of touched entries, never operation grants. Empty sets match all.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    pub source_ids: BTreeSet<String>,
    pub providers: BTreeSet<String>,
}

/// An optional stable caller proof. It is not a master key or a data HMAC.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppToken {
    pub version: u32,
    pub app: String,
    pub token: [u8; 32],
}
