//! Observational inputs; an explanation is never an execution capability.
use super::{
    PolicyRecord,
    effective::PolicyEvidence,
    record::Axis,
    requests::{Acquisition, Retrieval},
};
use crate::models::{claim::Scope, retrieval::SourceInput};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExplainRequest {
    Retrieve {
        request: Retrieval,
        scope: Option<Scope>,
    },
    Acquire {
        source: SourceInput,
        request: Acquisition,
        scope: Option<Scope>,
    },
    Compose {
        retrieval: Retrieval,
        acquisition: Acquisition,
        scope: Option<Scope>,
    },
    Claim {
        app: Uuid,
        claim: Uuid,
    },
}
impl ExplainRequest {
    pub fn source(&self) -> Option<&SourceInput> {
        match self {
            Self::Retrieve { request, .. } => Some(&request.source),
            Self::Acquire { source, .. } => Some(source),
            Self::Compose { retrieval, .. } => Some(&retrieval.source),
            Self::Claim { .. } => None,
        }
    }
    pub fn scope(&self) -> Option<&Scope> {
        match self {
            Self::Retrieve { scope, .. }
            | Self::Acquire { scope, .. }
            | Self::Compose { scope, .. } => scope.as_ref(),
            Self::Claim { .. } => None,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    BuiltIn,
    Source,
    Application,
    Request,
    AdoptedClaim,
}
#[derive(Debug, Clone, Serialize)]
pub struct AxisExplanation {
    pub applicable: bool,
    pub value: Option<serde_json::Value>,
    pub origin: Option<Origin>,
    pub evidence: Option<PolicyEvidence>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RestrictionEvidence {
    pub origin: Origin,
    pub record: PolicyRecord,
    pub digest: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct PolicyExplanation {
    pub axes: BTreeMap<Axis, AxisExplanation>,
    pub restrictions: Vec<RestrictionEvidence>,
}
