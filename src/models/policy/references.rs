//! Reference collections carry identity, not configuration or precedence.
use crate::error::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const MAX_POLICY_REFERENCES: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Uuid>", into = "Vec<Uuid>")]
pub struct PolicyRefs(Vec<Uuid>);

impl PolicyRefs {
    pub fn ids(&self) -> &[Uuid] {
        &self.0
    }
}
impl TryFrom<Vec<Uuid>> for PolicyRefs {
    type Error = Error;
    fn try_from(ids: Vec<Uuid>) -> Result<Self> {
        if ids.len() > MAX_POLICY_REFERENCES
            || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "duplicate or excessive policy references",
            ));
        }
        Ok(Self(ids))
    }
}
impl From<PolicyRefs> for Vec<Uuid> {
    fn from(value: PolicyRefs) -> Self {
        value.0
    }
}
