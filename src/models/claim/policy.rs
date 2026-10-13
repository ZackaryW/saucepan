use super::Scope;
use crate::{
    error::{Error, ErrorCode, Result},
    models::policy::{Policies, RevisionPolicy, UpdatePolicy},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Discover,
    Read,
    Acquire,
    Lease,
    Update,
    Recover,
    Release,
    Activate,
    Remove,
    SetContext,
    RegisterPolicy,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccessPolicy {
    pub sources: Option<BTreeSet<Uuid>>,
    pub scopes: Option<BTreeSet<Scope>>,
    pub actions: Option<BTreeSet<Action>>,
    pub public_scopes: BTreeSet<Scope>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReclaimStrategy {
    #[default]
    ReportUnavailable,
    RestoreExact,
    Refresh,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReclaimPolicy {
    ReclaimPolicy { strategy: ReclaimStrategy },
}
impl Default for ReclaimPolicy {
    fn default() -> Self {
        Self::ReclaimPolicy {
            strategy: ReclaimStrategy::default(),
        }
    }
}
impl ReclaimPolicy {
    pub fn strategy(&self) -> ReclaimStrategy {
        let Self::ReclaimPolicy { strategy } = self;
        *strategy
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Effective {
    pub output: Policies,
    pub reclaim: ReclaimPolicy,
    pub update_policy: UpdatePolicy,
}
impl Effective {
    pub fn validate(&self) -> Result<()> {
        if matches!(self.output.revision, RevisionPolicy::Latest { keep: 0 }) {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "retention must be positive",
            ));
        }
        Ok(())
    }
}
