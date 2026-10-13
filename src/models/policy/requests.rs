//! Reference-only operation inputs. Execution settings are derived by the planner.
use super::PolicyRefs;
use crate::models::{
    retrieval::SourceInput,
    store::{Destination, Format, Platform, Selector},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PathAccess {
    pub policy_refs: PolicyRefs,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AssetSelection {
    pub platform: Option<Platform>,
    pub filter_refs: PolicyRefs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retrieval {
    pub source: SourceInput,
    pub selector: Selector,
    #[serde(default)]
    pub asset: AssetSelection,
    #[serde(default)]
    pub policy_refs: PolicyRefs,
    pub destination: Option<PathBuf>,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Acquisition {
    pub destination: Destination,
    pub format: Format,
    pub filename: Option<String>,
    pub folder: Option<String>,
    pub bin: Vec<String>,
    pub policy_refs: PolicyRefs,
}
