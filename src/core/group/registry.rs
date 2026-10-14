use super::*;
use anyhow::Context;
impl Store {
    pub(crate) fn group_records_locked(&self) -> Result<Vec<GroupRecord>> {
        let snapshot = self.metadata.snapshot()?;
        snapshot
            .catalog
            .records
            .values()
            .filter(|r| r.identity.kind == Kind::Group)
            .map(|r| {
                let group: GroupRecord =
                    snapshot.read(self.metadata.authenticator(), r.identity)?;
                group.definition.validate()?;
                ensure!(
                    group.witnesses.len() == group.definition.members.len()
                        && group
                            .definition
                            .members
                            .iter()
                            .all(|m| group.witnesses.contains_key(&m.name)),
                    Error::new(
                        ErrorCode::IntegrityError,
                        "group authorization witnesses are incomplete"
                    )
                );
                ensure!(
                    identity(&group) == r.identity
                        && group.revision > 0
                        && group.definition == group.definition.canonical(),
                    Error::new(
                        ErrorCode::IntegrityError,
                        "group identity or definition mismatch"
                    )
                );
                Ok(group)
            })
            .collect()
    }
    pub(super) fn find_group(&self, uuid: Uuid) -> Result<GroupRecord> {
        self.group_records_locked()?
            .into_iter()
            .find(|g| g.uuid == uuid)
            .context(Error::new(
                ErrorCode::InvalidRequest,
                "unknown resource group",
            ))
    }
}
