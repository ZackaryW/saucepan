//! Results captured in the same coordinated operation as policy adoption.
use super::PolicyRefs;
use crate::models::claim::Claim;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationAdoption {
    pub app: Uuid,
    pub revision: u64,
    pub policy_refs: PolicyRefs,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimAdoption {
    pub app: Uuid,
    pub revision: u64,
    pub claim: Claim,
}
