//! Mutable selection/check state, independent of immutable output metadata.
use super::{AcquisitionRequest, OutputPlan};
use crate::models::{retrieval::VerificationEvidence, source::Resolved};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionState {
    pub uuid: Uuid,
    pub key: String,
    pub revision: u64,
    pub selected: Uuid,
    pub request: OutputPlan,
    pub acquisition: AcquisitionRequest,
    pub last_success: Option<u64>,
    pub checked_source: Option<Resolved>,
    pub verification: VerificationEvidence,
    #[serde(default)]
    pub owner: Option<AcquisitionOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionOwner {
    pub app: Uuid,
    pub scope: Option<crate::models::claim::Scope>,
    pub claim: Option<Uuid>,
}
