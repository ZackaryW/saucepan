use super::store::{OutputEntry, Request};
use crate::error::{Error, ErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;
pub mod lifecycle;
pub use lifecycle::{AppState, OwnedShim};

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub source_filter: Vec<Uuid>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub uuid: Uuid,
    pub request: Request,
    pub entry_uuid: Uuid,
    #[serde(default)]
    pub context: ClaimContext,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claimed {
    pub claim: Claim,
    pub acquired: super::store::Acquired,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimView {
    pub claim: Claim,
    pub state: super::store::Availability,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub entry_uuid: Uuid,
    pub executable: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub uuid: Uuid,
    pub name: String,
    pub settings: Settings,
    pub claims: BTreeMap<Uuid, Claim>,
    pub active: Option<Binding>,
    #[serde(default)]
    pub state: AppState,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub shims: Vec<OwnedShim>,
}
impl Application {
    pub(crate) fn bump(&mut self) -> anyhow::Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("application revision overflow"))?;
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    Application(Application),
    Output(Box<OutputEntry>),
}
impl Record {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Application(a) => a.uuid,
            Self::Output(e) => e.uuid,
        }
    }
}
