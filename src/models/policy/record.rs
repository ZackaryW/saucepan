//! Leaf definitions shared by persistent and caller-owned policy catalogs.
use super::{Freshness, Identical, PayloadVerification, RevisionPolicy, UpdatePolicy};
use crate::{
    error::{Error, ErrorCode, Result},
    models::{
        claim::{
            Scope,
            policy::{Action, ReclaimStrategy},
        },
        store::{AssetFilter, SelectionPolicy, VerificationMode},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRecord {
    pub uuid: Uuid,
    #[serde(flatten)]
    pub definition: PolicyDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PolicyDefinition {
    AccessPolicy(AccessRules),
    SourceVisibilityPolicy { public_discovery: bool },
    ClaimVisibilityPolicy { public_scopes: BTreeSet<Scope> },
    CachePolicy { mode: CacheMode },
    RefreshPolicy(Freshness),
    StalePolicy { allow_stale: bool },
    VerificationPolicy(VerificationPolicy),
    GithubReleaseAssetFilter(AssetFilter),
    SelectionPolicy(SelectionPolicy),
    IdenticalPullPolicy { strategy: Identical },
    UpdatePolicy(UpdatePolicy),
    RevisionPolicy(RevisionPolicy),
    ReclaimPolicy { strategy: ReclaimStrategy },
    UseableAppPolicy {},
    PinPolicy { retention: PinRetention },
    PolicyConstraints(Constraints),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinRetention {
    UntilReleased,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccessRules {
    pub sources: Option<BTreeSet<Uuid>>,
    pub scopes: Option<BTreeSet<Scope>>,
    pub actions: Option<BTreeSet<Action>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    #[default]
    Cached,
    Temporary,
    NoStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "policy_target", rename_all = "snake_case", deny_unknown_fields)]
pub enum VerificationPolicy {
    Payload { mode: PayloadVerification },
    Path { mode: VerificationMode },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Constraints {
    pub minimum_keep: usize,
    pub maximum_keep: usize,
    pub allowed_identical: Vec<Identical>,
    pub allowed_reclaim: Vec<ReclaimStrategy>,
    pub allow_useable_app: bool,
    pub require_useable_app: bool,
    pub allow_automatic_updates: bool,
    pub minimum_update_interval: u64,
}
impl Default for Constraints {
    fn default() -> Self {
        Self {
            minimum_keep: 1,
            maximum_keep: usize::MAX,
            allowed_identical: vec![Identical::Reuse, Identical::NewEntry],
            allowed_reclaim: vec![
                ReclaimStrategy::ReportUnavailable,
                ReclaimStrategy::RestoreExact,
                ReclaimStrategy::Refresh,
            ],
            allow_useable_app: true,
            require_useable_app: false,
            allow_automatic_updates: true,
            minimum_update_interval: 0,
        }
    }
}

/// Singleton behavior keys; restrictions instead intersect and filters are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    SourceVisibility,
    ClaimVisibility,
    Cache,
    Refresh,
    Stale,
    PayloadVerification,
    PathVerification,
    Selection,
    Identical,
    Update,
    Revision,
    Reclaim,
    Protection,
}
impl PolicyDefinition {
    pub fn axis(&self) -> Option<Axis> {
        use PolicyDefinition::*;
        Some(match self {
            AccessPolicy(_)
            | PolicyConstraints(_)
            | GithubReleaseAssetFilter(_)
            | PinPolicy { .. } => return None,
            SourceVisibilityPolicy { .. } => Axis::SourceVisibility,
            ClaimVisibilityPolicy { .. } => Axis::ClaimVisibility,
            CachePolicy { .. } => Axis::Cache,
            RefreshPolicy(_) => Axis::Refresh,
            StalePolicy { .. } => Axis::Stale,
            VerificationPolicy(crate::models::policy::record::VerificationPolicy::Payload {
                ..
            }) => Axis::PayloadVerification,
            VerificationPolicy(crate::models::policy::record::VerificationPolicy::Path {
                ..
            }) => Axis::PathVerification,
            SelectionPolicy(_) => Axis::Selection,
            IdenticalPullPolicy { .. } => Axis::Identical,
            UpdatePolicy(_) => Axis::Update,
            RevisionPolicy(_) => Axis::Revision,
            ReclaimPolicy { .. } => Axis::Reclaim,
            UseableAppPolicy {} => Axis::Protection,
        })
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::RevisionPolicy(policy) => {
                policy.keep()?;
            }
            Self::PolicyConstraints(bounds) => {
                if bounds.minimum_keep == 0
                    || bounds.minimum_keep > bounds.maximum_keep
                    || (bounds.require_useable_app && !bounds.allow_useable_app)
                {
                    return Err(Error::new(
                        ErrorCode::PolicyConflict,
                        "invalid policy constraints",
                    ));
                }
            }
            Self::GithubReleaseAssetFilter(filter) => {
                if filter.asset_patterns.len() > 256 {
                    return Err(Error::new(
                        ErrorCode::InvalidRequest,
                        "too many asset patterns",
                    ));
                }
                for pattern in &filter.asset_patterns {
                    if pattern.len() > 4096 {
                        return Err(Error::new(
                            ErrorCode::InvalidRequest,
                            "asset pattern is too long",
                        ));
                    }
                    globset::Glob::new(pattern).map_err(|_| {
                        Error::new(ErrorCode::InvalidRequest, "invalid asset pattern")
                    })?;
                }
            }
            Self::AccessPolicy(rules) => {
                if rules.sources.as_ref().is_some_and(|s| s.len() > 4096)
                    || rules.scopes.as_ref().is_some_and(|s| s.len() > 4096)
                {
                    return Err(Error::new(
                        ErrorCode::InvalidRequest,
                        "access policy is too large",
                    ));
                }
            }
            Self::ClaimVisibilityPolicy { public_scopes } if public_scopes.len() > 4096 => {
                return Err(Error::new(
                    ErrorCode::InvalidRequest,
                    "visibility policy is too large",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}
