//! Query transport types. Persisted context belongs to the claim model.
use crate::models::index::ClaimView;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Exact filters on stored source definitions, not provider discovery results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourceQuery {
    pub uuid: Option<Uuid>,
    #[serde(rename = "type")]
    pub kind: Option<SourceType>,
    pub repo: Option<String>,
    pub origin: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Local,
    Url,
    Git,
    GithubRelease,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceView {
    pub uuid: Uuid,
    pub source: crate::models::source::Source,
    pub public_discovery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceQueryResult {
    pub generation: u64,
    pub sources: Vec<SourceView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimQuery {
    pub app: Uuid,
    #[serde(default)]
    pub scope: Option<crate::models::claim::Scope>,
    #[serde(rename = "where")]
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryQuery {
    #[serde(default)]
    pub scope: Option<crate::models::claim::Scope>,
    #[serde(rename = "where")]
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub key: String,
    pub op: Operator,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Glob,
}

/// An owned metadata snapshot, not a resource lease or uniqueness assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryResult {
    pub app: Uuid,
    pub revision: u64,
    pub claims: Vec<ClaimView>,
}
