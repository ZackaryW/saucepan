//! Compile member ownership using the existing scoped claim matcher.
use super::*;
use crate::{
    core::claim::matching,
    models::{
        claim::Claim,
        group::{Member, Target},
        store::OutputPlan,
    },
};
use anyhow::Context;
pub(super) struct MemberIntent {
    pub request: OutputPlan,
    pub claim: Option<Claim>,
    pub relative: Option<String>,
    pub fixed: Option<Uuid>,
}
impl Store {
    pub(super) fn group_member_request(
        &self,
        group: &GroupRecord,
        member: &Member,
    ) -> Result<MemberIntent> {
        match &member.target {
            Target::Acquire(input) => {
                let request = self.group_acquisition_plan(group.owner, input)?;
                if let Some(app) = group.owner {
                    let intent = input.claim.as_ref().context("app claim intent missing")?;
                    let registry = self.registry(app)?;
                    let effective = self.claim_execution_policy(
                        request.source_uuid,
                        &registry.registry,
                        request.adopted.as_deref(),
                    )?;
                    if let Some(found) =
                        matching::select(self, &registry, &intent.scope, &request, &effective)?
                    {
                        ensure!(
                            found.claim.context == intent.context,
                            Error::new(
                                ErrorCode::ClaimContextConflict,
                                "existing group claim context differs"
                            )
                        );
                        return Ok(MemberIntent {
                            request: found.claim.request.clone(),
                            claim: Some(found.claim),
                            relative: found.relative,
                            fixed: None,
                        });
                    }
                    let claim = Claim {
                        uuid: Uuid::new_v4(),
                        scope: intent.scope.clone(),
                        context: intent.context.clone(),
                        request: request.clone(),
                        effective,
                        policy_refs: self.adopted_refs(request.adopted.as_deref())?,
                        entry_uuid: Uuid::nil(),
                    };
                    Ok(MemberIntent {
                        request,
                        claim: Some(claim),
                        relative: None,
                        fixed: None,
                    })
                } else {
                    Ok(MemberIntent {
                        request,
                        claim: None,
                        relative: None,
                        fixed: None,
                    })
                }
            }
            Target::Claim { uuid } => {
                let app = group.owner.context(Error::new(
                    ErrorCode::AccessDenied,
                    "claim group requires app owner",
                ))?;
                let claim = self
                    .registry(app)?
                    .claims
                    .get(uuid)
                    .context(Error::new(ErrorCode::UnknownClaim, "group claim missing"))?
                    .clone();
                self.authorize_claim(&self.registry(app)?, &claim, Action::Read)?;
                Ok(MemberIntent {
                    request: claim.request.clone(),
                    claim: Some(claim),
                    relative: None,
                    fixed: None,
                })
            }
            Target::Output { uuid } => {
                self.authorize_output(*uuid, Action::Read)?;
                self.authorize_output(*uuid, Action::Lease)?;
                let output = self.output(*uuid)?;
                let claim = group
                    .owner
                    .map(|app| {
                        let registry = self.registry(app)?;
                        registry
                            .claims
                            .values()
                            .find(|c| {
                                c.entry_uuid == *uuid
                                    && self.authorize_claim(&registry, c, Action::Lease).is_ok()
                            })
                            .cloned()
                            .context(Error::new(
                                ErrorCode::AccessDenied,
                                "fixed output has no authorized claim",
                            ))
                    })
                    .transpose()?;
                Ok(MemberIntent {
                    request: output.request,
                    claim,
                    relative: None,
                    fixed: Some(*uuid),
                })
            }
        }
    }
}
