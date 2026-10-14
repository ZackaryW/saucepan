use crate::{
    error::{Error, ErrorCode, Result},
    models::{
        policy::{PolicyRefs, effective::PolicyEvidence},
        store::VerificationMode,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "UncheckedGroup")]
pub struct GroupRequest {
    pub members: Vec<GroupMember>,
    #[serde(default)]
    pub policy_refs: PolicyRefs,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedGroup {
    members: Vec<GroupMember>,
    #[serde(default)]
    policy_refs: PolicyRefs,
}
impl TryFrom<UncheckedGroup> for GroupRequest {
    type Error = Error;
    fn try_from(value: UncheckedGroup) -> Result<Self> {
        let request = Self {
            members: value.members,
            policy_refs: value.policy_refs,
        };
        request.validate()?;
        Ok(request)
    }
}
impl GroupRequest {
    pub const MAX_MEMBERS: usize = 128;
    pub const MAX_JSON_BYTES: u64 = 1024 * 1024;

    /// Also applied at admission so native callers cannot bypass JSON validation.
    pub fn validate(&self) -> Result<()> {
        let invalid = || {
            Error::new(
                ErrorCode::InvalidRequest,
                "invalid group member name, count or directory",
            )
        };
        if self.members.is_empty() || self.members.len() > Self::MAX_MEMBERS {
            return Err(invalid());
        }
        let mut names = BTreeSet::new();
        for member in &self.members {
            let name = member.name.as_bytes();
            if name.is_empty()
                || name.len() > 64
                || !name[0].is_ascii_alphabetic()
                || !name
                    .iter()
                    .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
                || !names.insert(&member.name)
            {
                return Err(invalid());
            }
            if let Some(relative) = &member.relative {
                crate::utils::path::relative_path(relative).map_err(|_| invalid())?;
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMember {
    pub name: String,
    pub target: GroupTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupTarget {
    Output { uuid: Uuid },
    Claim { uuid: Uuid },
}

/// Paths and policy provenance only. This value never grants ownership or access.
#[derive(Debug, Clone, Serialize)]
pub struct GroupDescriptor {
    pub members: BTreeMap<String, GroupMemberDescriptor>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GroupMemberDescriptor {
    pub output_uuid: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_uuid: Option<Uuid>,
    pub directory: PathBuf,
    pub relative: Option<String>,
    pub verification: VerificationMode,
    pub policy_provenance: Vec<PolicyEvidence>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn request() -> Value {
        json!({"members":[{"name":"Command_1","target":{"type":"output","uuid":Uuid::nil()}}]})
    }
    #[test]
    fn documented_json_is_valid() {
        let blocks: Vec<_> = include_str!("../../../docs/group-leases.md")
            .split("```json\n")
            .skip(1)
            .map(|part| part.split("```").next().unwrap())
            .collect();
        assert_eq!(blocks.len(), 2);
        serde_json::from_str::<GroupRequest>(blocks[0]).unwrap();
        let records: Vec<crate::models::policy::PolicyRecord> =
            serde_json::from_str(blocks[1]).unwrap();
        crate::core::policy::catalog::PolicyCatalog::new(records).unwrap();
    }
    #[test]
    fn strict_group_contract() {
        let valid: GroupRequest = serde_json::from_value(request()).unwrap();
        assert!(valid.policy_refs.ids().is_empty());
        let round: GroupRequest =
            serde_json::from_value(serde_json::to_value(&valid).unwrap()).unwrap();
        assert_eq!(round.members[0].name, "Command_1");
        for name in ["", "1first", "a.b", "white space", "é", &"a".repeat(65)] {
            let mut value = request();
            value["members"][0]["name"] = json!(name);
            assert!(serde_json::from_value::<GroupRequest>(value).is_err());
        }
        for path in [
            "",
            "/absolute",
            "../escape",
            "a/../b",
            "a\\b",
            "C:/x",
            "a//b",
            "NUL",
        ] {
            let mut value = request();
            value["members"][0]["relative"] = json!(path);
            assert!(serde_json::from_value::<GroupRequest>(value).is_err());
        }
        for (pointer, value) in [
            ("/extra", json!(1)),
            ("/members/0/extra", json!(1)),
            ("/members/0/target/extra", json!(1)),
            ("/members/0/target/uuid", json!("broken")),
        ] {
            let mut input = request();
            let (parent, field) = pointer.rsplit_once('/').unwrap();
            input.pointer_mut(parent).unwrap()[field] = value;
            assert!(serde_json::from_value::<GroupRequest>(input).is_err());
        }
        for count in [0, 129] {
            let members: Vec<_> = (0..count).map(|i| json!({"name":format!("m{i}"),"target":{"type":"output","uuid":Uuid::nil()}})).collect();
            assert!(serde_json::from_value::<GroupRequest>(json!({"members":members})).is_err());
        }
        let mut duplicate = valid.clone();
        let mut maximum = valid.clone();
        maximum.members = (0..GroupRequest::MAX_MEMBERS)
            .map(|i| GroupMember {
                name: format!("member{i}"),
                target: GroupTarget::Output { uuid: Uuid::nil() },
                relative: None,
            })
            .collect();
        assert!(maximum.validate().is_ok());
        serde_json::from_value::<GroupRequest>(serde_json::to_value(maximum).unwrap()).unwrap();
        duplicate.members.push(valid.members[0].clone());
        assert!(duplicate.validate().is_err());
        duplicate.members[1].name = "command_1".into();
        assert!(duplicate.validate().is_ok());
        for refs in [
            vec![Uuid::nil(); 2],
            (0..257).map(|_| Uuid::new_v4()).collect(),
        ] {
            let mut input = request();
            input["policy_refs"] = json!(refs);
            assert!(serde_json::from_value::<GroupRequest>(input).is_err());
        }
    }
}
