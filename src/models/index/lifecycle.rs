use super::Application;
use crate::models::store::OutputPlan;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppState {
    #[default]
    Ready,
    Removing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedShim {
    pub name: String,
    pub sidecar: String,
    pub digest: String,
    pub config_digest: String,
    pub saucepan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub args: Vec<String>,
    pub stdout_contains: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub scope: crate::models::claim::Scope,
    pub name: String,
    pub request: crate::models::store::Request,
    pub probe: Probe,
    pub shim: String,
    pub saucepan: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShimAction {
    Install,
    Replace,
    Adopt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Install {
        scope: crate::models::claim::Scope,
        name: String,
        request: Box<OutputPlan>,
        probe: Probe,
        shim: String,
        digest: String,
        saucepan: bool,
    },
    Update {
        claim: Uuid,
        probe: Probe,
        rollback: Option<Uuid>,
    },
    Shim {
        action: ShimAction,
        name: String,
        digest: String,
        saucepan: bool,
    },
    Uninstall {
        leave_unmanaged_shims: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Preparing,
    CommitIntent,
    CleanupPending,
    Complete,
    Failed,
    Conflict,
}
impl Phase {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Conflict)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShimChange {
    pub previous: Option<OwnedShim>,
    pub desired: OwnedShim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    #[serde(default)]
    pub source_policy: Option<SourcePolicyObservation>,
    pub operation: Uuid,
    pub app: Uuid,
    pub action: Action,
    pub fingerprint: String,
    pub status: Phase,
    pub changed: bool,
    pub expected_revision: Option<u64>,
    pub claim: Uuid,
    pub candidate: Option<Uuid>,
    pub desired: Option<Application>,
    pub desired_registry: Option<crate::models::claim::RegistryState>,
    pub shims: Vec<ShimChange>,
    pub pending: Vec<String>,
    pub reason: Option<crate::error::Error>,
    pub leftovers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicyObservation {
    pub source_uuid: Uuid,
    pub revision: u64,
}
