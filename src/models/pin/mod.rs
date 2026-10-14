//! Durable retention is explicit and independent of mutable claim selection.
use crate::{
    error::{Error, ErrorCode, Result},
    models::{claim::Scope, policy::PolicyRefs},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRequest {
    pub registration_key: Scope,
    pub outputs: Vec<Uuid>,
    pub policy_refs: PolicyRefs,
}
impl PinRequest {
    pub fn validate(&self) -> Result<()> {
        if self.outputs.is_empty()
            || self.outputs.len() > 128
            || self.outputs.iter().collect::<BTreeSet<_>>().len() != self.outputs.len()
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "pin requires 1..128 distinct outputs",
            ));
        }
        Ok(())
    }
    pub(crate) fn canonical(&self) -> Self {
        let mut value = self.clone();
        value.outputs.sort();
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinState {
    Active,
    Released,
}

/// Captured from an actual authorized claim. It survives subsequent claim movement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinWitness {
    pub output_uuid: Uuid,
    pub source_uuid: Uuid,
    pub scope: Option<Scope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRecord {
    pub uuid: Uuid,
    pub owner: Option<Uuid>,
    pub request: PinRequest,
    pub witnesses: Vec<PinWitness>,
    pub state: PinState,
    pub group: Option<GroupPinReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupPinRequest {
    pub registration_key: Scope,
    pub policy_refs: PolicyRefs,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupPinOperation {
    Prepare,
    Pin,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupPinReceipt {
    pub operation: GroupPinOperation,
    pub expected_revision: u64,
    pub group: crate::models::group::GroupRecord,
    pub stale: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PinnedOutputs {
    pub pin: PinRecord,
    pub paths: std::collections::BTreeMap<Uuid, PathBuf>,
}
