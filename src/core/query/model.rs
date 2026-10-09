//! Query transport types. Persisted context belongs to the claim model.
use crate::models::index::ClaimView;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
