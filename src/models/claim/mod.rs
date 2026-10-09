//! Contracts for the separate claim registry. Persistence is owned by core::claim.
use crate::{
    error::{Error, ErrorCode, Result},
    models::store::Request,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;
pub mod policy;
pub mod security;

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
    pub uuid: Uuid,
    pub scope: Scope,
    pub request: Request,
    pub entry_uuid: Uuid,
    pub context: super::index::ClaimContext,
    pub overrides: policy::Overrides,
    pub effective: policy::Effective,
}
impl Claim {
    pub fn validate(&self) -> Result<()> {
        super::index::validate_context(&self.context)
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "invalid claim context"))?;
        self.effective.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub app: Uuid,
    pub revision: u64,
    pub registration_key: Scope,
    pub access: policy::AccessPolicy,
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
