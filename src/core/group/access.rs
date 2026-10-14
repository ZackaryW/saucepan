use super::*;
use crate::{
    core::policy::catalog::PolicyCatalog,
    models::lease::{GroupMember, GroupRequest, GroupTarget},
};
use anyhow::Context;
impl Store {
    /// Open one exact published generation. This never fetches or repairs a claim.
    pub fn open_group(&self, uuid: Uuid) -> Result<PreparedGroup> {
        let (group, request) = {
            let _lock = self.lock()?;
            let group = self.find_group(uuid)?;
            self.authorize_group(&group, Action::Lease)?;
            let selected = group.selected.as_ref().context(Error::new(
                ErrorCode::ContentUnavailable,
                "group has no prepared generation",
            ))?;
            let mut members = Vec::new();
            for (name, member) in &selected.members {
                let target = if let (Some(app), Some(claim)) = (group.owner, member.claim_uuid) {
                    let registry = self.registry(app)?;
                    let current = registry.claims.get(&claim).context(Error::new(
                        ErrorCode::RevisionConflict,
                        "group claim was released; prepare again",
                    ))?;
                    ensure!(
                        current.entry_uuid == member.output_uuid
                            && Some(&current.scope) == member.scope.as_ref(),
                        Error::new(
                            ErrorCode::RevisionConflict,
                            "group claim selection changed; prepare again"
                        )
                    );
                    GroupTarget::Claim { uuid: claim }
                } else {
                    GroupTarget::Output {
                        uuid: member.output_uuid,
                    }
                };
                members.push(GroupMember {
                    name: name.clone(),
                    target,
                    relative: member.relative.clone(),
                });
            }
            let request = GroupRequest {
                members,
                policy_refs: group.definition.policy_refs.clone(),
            };
            (group, request)
        };
        let lease = self.lease_group(&request, &PolicyCatalog::default())?;
        let _lock = self.lock()?;
        ensure!(
            self.find_group(uuid)?.revision == group.revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "group changed during admission"
            )
        );
        Ok(PreparedGroup {
            group,
            lease,
            pin: None,
            stale: Default::default(),
        })
    }
}
impl AppSession {
    pub fn open_group(&self, uuid: Uuid) -> Result<PreparedGroup> {
        self.store.open_group(uuid)
    }
}
