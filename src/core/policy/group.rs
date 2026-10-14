//! Voluntary request boundaries are intersections, never request-level grants.
use super::{catalog::PolicyCatalog, resolve};
use crate::{
    error::{Error, ErrorCode, Result},
    models::{
        claim::{Scope, policy::Action},
        policy::{
            PolicyDefinition, PolicyRefs,
            effective::{EvaluatedPolicies, PolicyEvidence},
            record::VerificationPolicy,
        },
        store::VerificationMode,
    },
};
use uuid::Uuid;

/// A pure narrowing boundary. Callers must establish current authority separately.
pub struct GroupBoundary {
    plan: EvaluatedPolicies,
    floor: Option<VerificationMode>,
    pub evidence: Vec<PolicyEvidence>,
}
impl GroupBoundary {
    pub fn evaluate(catalog: &PolicyCatalog, refs: &PolicyRefs) -> Result<Self> {
        let mut result = Self {
            plan: EvaluatedPolicies::default(),
            floor: None,
            evidence: Vec::new(),
        };
        for id in refs.ids() {
            let record = catalog.get(*id)?;
            match &record.definition {
                PolicyDefinition::AccessPolicy(rules) => result.plan.access.push(rules.clone()),
                PolicyDefinition::VerificationPolicy(VerificationPolicy::Path { mode }) => {
                    if result.floor.replace(*mode).is_some() {
                        return Err(Error::new(
                            ErrorCode::PolicyConflict,
                            "multiple group path verification policies",
                        ));
                    }
                }
                _ => {
                    return Err(Error::new(
                        ErrorCode::InvalidRequest,
                        "group boundaries accept only access and path verification policies",
                    ));
                }
            }
            result.evidence.push(resolve::evidence(record)?);
        }
        Ok(result)
    }
    pub fn authorize(&self, source: Uuid, scope: Option<&Scope>) -> Result<()> {
        self.authorize_action(Action::Lease, source, scope)
    }
    pub(crate) fn authorize_action(
        &self,
        action: Action,
        source: Uuid,
        scope: Option<&Scope>,
    ) -> Result<()> {
        resolve::authorize(&self.plan, action, Some(source), scope)
    }
    pub fn verification(&self, member: VerificationMode) -> VerificationMode {
        strongest(member, self.floor.unwrap_or(VerificationMode::Metadata))
    }
}
pub(crate) fn strongest(a: VerificationMode, b: VerificationMode) -> VerificationMode {
    if a == VerificationMode::Full || b == VerificationMode::Full {
        VerificationMode::Full
    } else {
        VerificationMode::Metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn catalog(values: Vec<serde_json::Value>) -> (PolicyCatalog, PolicyRefs) {
        let records: Vec<_> = values
            .into_iter()
            .map(|mut value| {
                value["uuid"] = json!(Uuid::new_v4());
                serde_json::from_value::<crate::models::policy::PolicyRecord>(value).unwrap()
            })
            .collect();
        let refs = records
            .iter()
            .map(|r| r.uuid)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        (PolicyCatalog::new(records).unwrap(), refs)
    }
    #[test]
    fn boundaries_intersect_and_cannot_invent_scope() {
        let source = Uuid::new_v4();
        let scope = Scope::try_from("project".to_owned()).unwrap();
        let (cat, refs) = catalog(vec![
            json!({"type":"access_policy","sources":[source]}),
            json!({"type":"access_policy","scopes":[scope],"actions":["lease"]}),
        ]);
        let boundary = GroupBoundary::evaluate(&cat, &refs).unwrap();
        assert!(boundary.authorize(source, Some(&scope)).is_ok());
        assert!(boundary.authorize(source, None).is_err());
        assert!(boundary.authorize(Uuid::new_v4(), Some(&scope)).is_err());
        for dimension in ["sources", "scopes", "actions"] {
            let mut value = json!({"type":"access_policy"});
            value[dimension] = json!([]);
            let (cat, refs) = catalog(vec![value]);
            assert!(
                GroupBoundary::evaluate(&cat, &refs)
                    .unwrap()
                    .authorize(source, Some(&scope))
                    .is_err()
            );
        }
        let (cat, refs) = catalog(vec![json!({"type":"access_policy"})]);
        assert!(
            GroupBoundary::evaluate(&cat, &refs)
                .unwrap()
                .authorize(source, None)
                .is_ok()
        );
        assert!(cat.with_portable(&cat).is_err());
        assert!(
            resolve::evaluate(
                &cat,
                resolve::Inputs {
                    source: &Default::default(),
                    application: &Default::default(),
                    request: &refs,
                    pinned: None,
                    operation: resolve::Operation::Read,
                    managed_output: false,
                    update_due: false
                }
            )
            .is_err()
        );
    }
    #[test]
    fn verification_is_a_floor_and_other_roles_are_rejected() {
        let path = json!({"type":"verification_policy","policy_target":"path","mode":"metadata"});
        let (cat, refs) = catalog(vec![path.clone()]);
        let boundary = GroupBoundary::evaluate(&cat, &refs).unwrap();
        assert_eq!(
            boundary.verification(VerificationMode::Full),
            VerificationMode::Full
        );
        let (cat, refs) = catalog(vec![path.clone(), path]);
        assert!(GroupBoundary::evaluate(&cat, &refs).is_err());
        for value in [
            json!({"type":"verification_policy","policy_target":"payload","mode":"none"}),
            json!({"type":"cache_policy","mode":"cached"}),
        ] {
            let (cat, refs) = catalog(vec![value]);
            assert!(GroupBoundary::evaluate(&cat, &refs).is_err());
        }
    }
}
