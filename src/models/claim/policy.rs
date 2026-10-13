use super::Scope;
use crate::{
    error::{Error, ErrorCode, Result},
    models::policy::{Identical, Policies, RevisionPolicy, UpdatePolicy},
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
    pub identical: Option<Identical>,
    pub revision: Option<RevisionPolicy>,
    pub reclaim: Option<ReclaimPolicy>,
    /// Output protection is registered by the app; overrides cannot remove it.
    pub useable_app: Option<bool>,
    pub update_policy: Option<UpdatePolicy>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Registration {
    pub defaults: Effective,
    pub minimum_keep: usize,
    pub maximum_keep: usize,
    pub allowed_identical: Vec<Identical>,
    pub allowed_reclaim: Vec<ReclaimStrategy>,
    pub allow_useable_app: bool,
    pub allow_automatic_updates: bool,
    pub minimum_update_interval: u64,
}
impl Default for Registration {
    fn default() -> Self {
        Self {
            defaults: Effective::default(),
            minimum_keep: 1,
            maximum_keep: usize::MAX,
            allowed_identical: vec![Identical::Reuse, Identical::NewEntry],
            allowed_reclaim: vec![
                ReclaimStrategy::ReportUnavailable,
                ReclaimStrategy::RestoreExact,
                ReclaimStrategy::Refresh,
            ],
            allow_useable_app: true,
            allow_automatic_updates: true,
            minimum_update_interval: 0,
        }
    }
}
impl Registration {
    pub fn validate(&self) -> Result<()> {
        self.defaults.validate()?;
        self.validate_update(self.defaults.update_policy)?;
        let keep = self.defaults.output.revision.keep()?;
        if self.minimum_keep == 0
            || self.minimum_keep > self.maximum_keep
            || !(self.minimum_keep..=self.maximum_keep).contains(&keep)
            || !self
                .allowed_identical
                .contains(&self.defaults.output.identical)
            || !self
                .allowed_reclaim
                .contains(&self.defaults.reclaim.strategy())
            || (!self.allow_useable_app && self.defaults.output.protection.is_some())
        {
            return Err(Error::new(
                ErrorCode::PolicyConflict,
                "invalid registered policy defaults or bounds",
            ));
        }
        Ok(())
    }
    pub fn evaluate(&self, overrides: &Overrides) -> Result<Effective> {
        self.validate()?;
        let mut effective = self.defaults.clone();
        if let Some(value) = overrides.identical {
            effective.output.identical = value;
        }
        if let Some(revision) = overrides.revision {
            effective.output.revision = revision;
        }
        if let Some(reclaim) = &overrides.reclaim {
            effective.reclaim = reclaim.clone();
        }
        if let Some(update_policy) = overrides.update_policy {
            effective.update_policy = update_policy;
        }
        self.validate_update(effective.update_policy)?;
        if let Some(protect) = overrides.useable_app {
            if (!protect && self.defaults.output.protection.is_some())
                || (protect && !self.allow_useable_app)
            {
                return Err(Error::new(
                    ErrorCode::PolicyConflict,
                    "protection override is not permitted",
                ));
            }
            effective.output.protection =
                protect.then_some(crate::models::policy::Protection::UseableApp);
        }
        let keep = effective
            .output
            .revision
            .keep()
            .map_err(|_| Error::new(ErrorCode::PolicyConflict, "invalid revision policy"))?;
        if !(self.minimum_keep..=self.maximum_keep).contains(&keep)
            || !self.allowed_identical.contains(&effective.output.identical)
            || !self.allowed_reclaim.contains(&effective.reclaim.strategy())
        {
            return Err(Error::new(
                ErrorCode::PolicyConflict,
                "claim override is outside registered allowances",
            ));
        }
        Ok(effective)
    }

    fn validate_update(&self, update: UpdatePolicy) -> Result<()> {
        let interval = match update {
            UpdatePolicy::Manual {} => return Ok(()),
            UpdatePolicy::Always {} => 0,
            UpdatePolicy::MaxAge { seconds } => seconds,
        };
        if !self.allow_automatic_updates || interval < self.minimum_update_interval {
            return Err(Error::new(
                ErrorCode::PolicyConflict,
                "update policy is outside registered allowances",
            ));
        }
        Ok(())
    }
}
