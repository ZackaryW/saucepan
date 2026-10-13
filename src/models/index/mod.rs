use super::store::OutputEntry;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod lifecycle;
pub use lifecycle::{AppState, OwnedShim};

pub use crate::models::claim::{
    Claim, ClaimContext, ClaimView, Claimed, validate_context, validate_context_field,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub source_filter: Vec<Uuid>,
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
    Retrieval(Box<super::retrieval::RetrievalRecord>),
    Acquisition(Box<super::store::AcquisitionState>),
}
impl Record {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Application(a) => a.uuid,
            Self::Output(e) => e.uuid,
            Self::Retrieval(e) => e.uuid,
            Self::Acquisition(e) => e.uuid,
        }
    }
}
