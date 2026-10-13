//! Contracts for the separate claim registry. Persistence is owned by core::claim.
use crate::{
    error::{Error, ErrorCode, Result},
    models::store::Request,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;
pub mod adopted;
pub mod policy;
pub mod request;
pub mod security;

/// Caller-owned labels; never part of output identity or acquisition policy.
pub type ClaimContext = BTreeMap<String, String>;

pub fn validate_context_field(key: &str, value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !key.is_empty() && key.len() <= 128,
        Error::new(
            ErrorCode::InvalidRequest,
            "context key must contain 1..=128 UTF-8 bytes"
        )
    );
    anyhow::ensure!(
        value.len() <= 4096,
        Error::new(
            ErrorCode::InvalidRequest,
            "context value exceeds 4096 UTF-8 bytes"
        )
    );
    Ok(())
}

pub fn validate_context(context: &ClaimContext) -> anyhow::Result<()> {
    anyhow::ensure!(
        context.len() <= 64,
        Error::new(ErrorCode::InvalidRequest, "context exceeds 64 entries")
    );
    for (key, value) in context {
        validate_context_field(key, value)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claimed {
    pub claim: Claim,
    pub view: CoveredView,
    pub acquired: super::store::Acquired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsureRequest {
    #[serde(default)]
    pub adopted: Option<Box<adopted::AdoptedPolicies>>,
    pub scope: Scope,
    pub source_uuid: Uuid,
    pub selector: super::store::Selector,
    #[serde(default)]
    pub selection: super::store::Selection,
    #[serde(default)]
    pub context: ClaimContext,
    #[serde(default)]
    pub overrides: policy::Overrides,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimView {
    pub claim: Claim,
    pub state: super::store::Availability,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Scope(String);
impl Scope {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Scope {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        if value.is_empty() || value.len() > 4096 {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "scope must contain 1..=4096 UTF-8 bytes",
            ));
        }
        Ok(Self(value))
    }
}
impl From<Scope> for String {
    fn from(scope: Scope) -> Self {
        scope.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    #[serde(default)]
    pub policy_refs: crate::models::policy::PolicyRefs,
    pub uuid: Uuid,
    pub scope: Scope,
    pub request: Request,
    pub entry_uuid: Uuid,
    pub context: ClaimContext,
    pub overrides: policy::Overrides,
    pub effective: policy::Effective,
}
impl Claim {
    pub fn validate(&self) -> Result<()> {
        validate_context(&self.context)
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "invalid claim context"))?;
        self.effective.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    #[serde(default)]
    pub policy_refs: crate::models::policy::PolicyRefs,
    pub app: Uuid,
    pub revision: u64,
    pub registration_key: Scope,
    #[serde(skip)]
    pub access: policy::AccessPolicy,
    #[serde(skip)]
    pub policies: policy::Registration,
    pub credential: security::CredentialPolicy,
    /// Expected member digests; payloads live in separate files.
    pub members: BTreeMap<Uuid, String>,
}
impl Registry {
    pub fn validate(&self) -> Result<()> {
        self.policies.validate()?;
        for digest in self.members.values() {
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::new(
                    ErrorCode::IntegrityError,
                    "invalid member digest",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoveredView {
    pub app: Uuid,
    pub claim: Uuid,
    pub entry: Uuid,
    /// None denotes the whole claimed output; Some is a portable relative path.
    pub relative: Option<String>,
}
impl CoveredView {
    pub fn validate(&self) -> Result<()> {
        if let Some(path) = &self.relative {
            crate::utils::path::relative_path(path)
                .map_err(|_| Error::new(ErrorCode::InvalidRequest, "invalid claim view path"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryState {
    pub registry: Registry,
    pub claims: BTreeMap<Uuid, Claim>,
}
