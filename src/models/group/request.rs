use crate::{
    error::{Error, Result},
    models::{
        claim::{ClaimContext, Scope, request::Selection},
        lease::{GroupMember, GroupRequest, GroupTarget},
        policy::PolicyRefs,
        store::Selector,
    },
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedDefinition")]
pub struct GroupDefinition {
    pub registration_key: Scope,
    pub members: Vec<Member>,
    pub policy_refs: PolicyRefs,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedDefinition {
    registration_key: Scope,
    members: Vec<Member>,
    #[serde(default)]
    policy_refs: PolicyRefs,
}
impl TryFrom<UncheckedDefinition> for GroupDefinition {
    type Error = Error;
    fn try_from(value: UncheckedDefinition) -> Result<Self> {
        let value = Self {
            registration_key: value.registration_key,
            members: value.members,
            policy_refs: value.policy_refs,
        };
        value.validate()?;
        Ok(value)
    }
}
impl GroupDefinition {
    pub fn validate(&self) -> Result<()> {
        GroupRequest {
            members: self
                .members
                .iter()
                .map(|m| GroupMember {
                    name: m.name.clone(),
                    target: GroupTarget::Output { uuid: Uuid::nil() },
                    relative: m.relative.clone(),
                })
                .collect(),
            policy_refs: self.policy_refs.clone(),
        }
        .validate()?;
        for member in &self.members {
            if let Target::Acquire(input) = &member.target {
                if let Some(claim) = &input.claim {
                    crate::models::claim::validate_context(&claim.context)
                        .map_err(|e| Error::from_anyhow(&e))?;
                }
                if let Some(folder) = &input.selection.folder {
                    crate::utils::path::relative_path(folder).map_err(Error::from)?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn canonical(&self) -> Self {
        let mut result = self.clone();
        result.members.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub target: Target,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Output { uuid: Uuid },
    Claim { uuid: Uuid },
    Acquire(Box<AcquireMember>),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquireMember {
    pub source_uuid: Uuid,
    pub selector: Selector,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub policy_refs: PolicyRefs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<ClaimIntent>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimIntent {
    pub scope: Scope,
    #[serde(default)]
    pub context: ClaimContext,
}
