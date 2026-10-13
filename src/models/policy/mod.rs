use serde::{Deserialize, Serialize};
mod composition;
pub use composition::{Freshness, PayloadVerification, RevisionPolicy, UpdatePolicy};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Identical {
    #[default]
    Reuse,
    NewEntry,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case", deny_unknown_fields)]
pub enum Eviction {
    Latest { keep: usize },
}
impl Default for Eviction {
    fn default() -> Self {
        Self::Latest { keep: 10 }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Protection {
    UseableApp,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policies {
    pub identical: Identical,
    pub eviction: Eviction,
    pub protection: Option<Protection>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refresh {
    pub ttl_seconds: u64,
}
impl Default for Refresh {
    fn default() -> Self {
        Self {
            ttl_seconds: 86_400,
        }
    }
}
