use super::store::{OutputEntry, Request};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

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
