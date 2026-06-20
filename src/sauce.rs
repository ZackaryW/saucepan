use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sauce {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
