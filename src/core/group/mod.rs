//! Persistent group ownership and orchestration; native use guards live in lease.
mod registry;
mod validation;
use crate::{
    core::{
        Store,
        claim::{
            security::{Identity, Kind},
            session::AppSession,
        },
        index::Journal,
    },
    error::{Error, ErrorCode},
    models::{
        claim::policy::Action,
        group::{GroupDefinition, GroupRecord},
        index::Record,
    },
};
use anyhow::{Result, ensure};
use uuid::Uuid;
pub(crate) fn identity(group: &GroupRecord) -> Identity {
    Identity {
        kind: Kind::Group,
        owner: group.owner,
        member: Some(group.uuid),
    }
}

impl Store {
    pub fn register_group(&self, definition: &GroupDefinition) -> Result<GroupRecord> {
        definition.validate()?;
        let definition = definition.canonical();
        let _lock = self.lock()?;
        let owner = self.session.as_ref().map(|s| s.app);
        let witnesses = self.validate_group(owner, &definition)?;
        if let Some(existing) = self.group_records_locked()?.into_iter().find(|g| {
            g.owner == owner && g.definition.registration_key == definition.registration_key
        }) {
            ensure!(
                existing.definition == definition,
                Error::new(
                    ErrorCode::RevisionConflict,
                    "group key has a different definition"
                )
            );
            return Ok(existing);
        }
        let record = GroupRecord {
            uuid: Uuid::new_v4(),
            owner,
            revision: 1,
            definition,
            witnesses,
            selected: None,
        };
        self.commit_group(record.clone(), false)?;
        Ok(record)
    }
    pub fn group(&self, uuid: Uuid) -> Result<GroupRecord> {
        let _lock = self.lock()?;
        let group = self.find_group(uuid)?;
        self.authorize_group(&group, Action::Read)?;
        Ok(group)
    }
    pub fn groups(&self, key: Option<&str>) -> Result<Vec<GroupRecord>> {
        let _lock = self.lock()?;
        if let Some(session) = &self.session {
            self.check_session(&self.registry(session.app)?)?;
        }
        Ok(self
            .group_records_locked()?
            .into_iter()
            .filter(|g| {
                key.is_none_or(|key| g.definition.registration_key.as_str() == key)
                    && self.authorize_group(g, Action::Read).is_ok()
            })
            .collect())
    }
    pub fn replace_group(
        &self,
        uuid: Uuid,
        expected_revision: u64,
        definition: &GroupDefinition,
    ) -> Result<GroupRecord> {
        definition.validate()?;
        let _lock = self.lock()?;
        let mut group = self.find_group(uuid)?;
        ensure!(
            group.revision == expected_revision,
            Error::new(ErrorCode::RevisionConflict, "group revision changed")
        );
        self.authorize_group(&group, Action::Release)?;
        let witnesses = self.validate_group(group.owner, definition)?;
        ensure!(
            group.definition.registration_key == definition.registration_key,
            Error::new(
                ErrorCode::InvalidRequest,
                "group registration key is immutable"
            )
        );
        let definition = definition.canonical();
        if group.definition != definition {
            group.definition = definition;
            group.witnesses = witnesses;
            group.selected = None;
            group.revision = group
                .revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("group revision overflow"))?;
            self.commit_group(group.clone(), false)?;
        }
        Ok(group)
    }
    pub fn remove_group(&self, uuid: Uuid, expected_revision: u64) -> Result<()> {
        let _lock = self.lock()?;
        let group = self.find_group(uuid)?;
        ensure!(
            group.revision == expected_revision,
            Error::new(ErrorCode::RevisionConflict, "group revision changed")
        );
        self.authorize_group(&group, Action::Release)?;
        self.commit_group(group, true)
    }
    fn commit_group(&self, group: GroupRecord, remove: bool) -> Result<()> {
        let mut journal = Journal::default();
        if let Some(app) = group.owner {
            let mut app = self.application(app)?;
            app.revision = app
                .revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("application revision overflow"))?;
            journal.records.push(Record::Application(app));
        }
        if remove {
            journal.remove_groups.push(identity(&group));
        } else {
            journal.groups.push(group);
        }
        self.commit(journal)
    }
}
impl AppSession {
    pub fn register_group(&self, definition: &GroupDefinition) -> Result<GroupRecord> {
        self.store.register_group(definition)
    }
    pub fn group(&self, uuid: Uuid) -> Result<GroupRecord> {
        self.store.group(uuid)
    }
    pub fn groups(&self, key: Option<&str>) -> Result<Vec<GroupRecord>> {
        self.store.groups(key)
    }
    pub fn replace_group(
        &self,
        uuid: Uuid,
        revision: u64,
        definition: &GroupDefinition,
    ) -> Result<GroupRecord> {
        self.store.replace_group(uuid, revision, definition)
    }
    pub fn remove_group(&self, uuid: Uuid, revision: u64) -> Result<()> {
        self.store.remove_group(uuid, revision)
    }
}
