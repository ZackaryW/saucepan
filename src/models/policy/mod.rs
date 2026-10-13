use serde::{Deserialize, Serialize};
pub mod adoption;
mod composition;
pub mod effective;
pub mod record;
pub mod references;
pub mod requests;
pub use composition::{Freshness, PayloadVerification, RevisionPolicy, UpdatePolicy};
pub use record::{PolicyDefinition, PolicyRecord};
pub use references::PolicyRefs;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Identical {
    #[default]
    Reuse,
    NewEntry,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Protection {
    UseableApp,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policies {
    pub identical: Identical,
    pub revision: RevisionPolicy,
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
