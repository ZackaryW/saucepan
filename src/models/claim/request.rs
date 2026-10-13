use super::{ClaimContext, Scope};
use crate::models::{
    policy::PolicyRefs,
    store::{Format, Platform, Selector},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub platform: Option<Platform>,
    pub filter_refs: PolicyRefs,
    pub format: Format,
    pub filename: Option<String>,
    pub folder: Option<String>,
    pub bin: Vec<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ensure {
    pub scope: Scope,
    pub source_uuid: Uuid,
    pub selector: Selector,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub context: ClaimContext,
    #[serde(default)]
    pub policy_refs: PolicyRefs,
}
