//! Setup owns preparation; the existing application lifecycle owns publication.
use super::{
    Store,
    claim::security::{Identity, Kind},
};
use crate::{
    error::{Error, ErrorCode},
    models::setup::{Record, SCHEMA_VERSION},
    utils::path::beneath,
};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;
use uuid::Uuid;
mod inspection;
mod migration;
pub use inspection::inspect;
mod preparation;
mod run;
pub use run::{Options, setup};
#[cfg(test)]
mod tests;

fn identity(operation: Uuid) -> Identity {
    Identity {
        kind: Kind::Setup,
        owner: None,
        member: Some(operation),
    }
}

impl Store {
    fn setup_admin(&self) -> Result<()> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "setup requires trusted administration"
            )
        );
        Ok(())
    }

    pub(crate) fn setup_dir(&self, operation: Uuid) -> Result<PathBuf> {
        Ok(beneath(&self.root, &format!(".staging/setup/{operation}"))?)
    }

    fn validate_setup(&self, record: &Record) -> Result<()> {
        ensure!(
            record.schema_version == SCHEMA_VERSION
                && record.result.schema_version == SCHEMA_VERSION
                && record.inputs.store == self.metadata.authenticator().store()
                && record.inputs.root == self.root
                && record.result.operation == record.operation
                && record.result.root == self.root
                && record.result.lifecycle_operation == Some(record.lifecycle_operation)
                && record.request.selector
                    == crate::models::store::Selector::Tag {
                        tag: record.inputs.tag.clone()
                    }
                && record.request.policy_refs == record.profile.combined_policy_refs
                && record.request.selection.filter_refs.ids() == [record.filter.uuid]
                && record.fingerprint == super::store::json_digest(&record.inputs)?
                && record.shim == self.setup_dir(record.operation)?.join("shim"),
            Error::new(
                ErrorCode::IntegrityError,
                "setup identity or inputs mismatch"
            )
        );
        if let Some(migration) = &record.migration {
            let backup = migration
                .backup
                .strip_prefix(&self.root)
                .context("migration backup is outside the owning store")?;
            ensure!(
                record.inputs.migrate_unmanaged
                    && record.inputs.expected_sha256.as_deref() == Some(&migration.digest)
                    && migration.source == self.root.join("bin").join(preparation::launcher_name())
                    && backup.starts_with(".staging/setup")
                    && migration.backup
                        == beneath(&self.root, &backup.to_string_lossy().replace('\\', "/"))?,
                Error::new(
                    ErrorCode::IntegrityError,
                    "migration evidence differs from consent"
                )
            );
        }
        Ok(())
    }

    pub fn setup_record(&self, operation: Uuid) -> Result<Record> {
        self.setup_admin()?;
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let record: Record = snapshot.read(self.metadata.authenticator(), identity(operation))?;
        ensure!(
            record.operation == operation,
            Error::new(ErrorCode::IntegrityError, "setup operation mismatch")
        );
        self.validate_setup(&record)?;
        Ok(record)
    }

    pub fn setup_records(&self) -> Result<Vec<Record>> {
        self.setup_admin()?;
        self.setup_records_internal()
    }

    pub(crate) fn setup_records_internal(&self) -> Result<Vec<Record>> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let mut records = Vec::new();
        for reference in snapshot
            .catalog
            .records
            .values()
            .filter(|r| r.identity.kind == Kind::Setup)
        {
            let record: Record =
                snapshot.read(self.metadata.authenticator(), reference.identity)?;
            ensure!(
                reference.identity == identity(record.operation),
                Error::new(ErrorCode::IntegrityError, "setup record identity mismatch")
            );
            self.validate_setup(&record)?;
            records.push(record);
        }
        records.sort_by_key(|record| record.operation);
        Ok(records)
    }

    pub(crate) fn save_setup(&self, record: &Record) -> Result<()> {
        self.setup_admin()?;
        ensure!(
            !self.metadata_only,
            "read-only setup inspection cannot publish"
        );
        self.validate_setup(record)?;
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        if snapshot.contains(identity(record.operation))? {
            let existing: Record =
                snapshot.read(self.metadata.authenticator(), identity(record.operation))?;
            ensure!(
                existing.fingerprint == record.fingerprint
                    && existing.request == record.request
                    && existing.profile == record.profile
                    && existing.filter == record.filter
                    && existing.lifecycle_operation == record.lifecycle_operation,
                "setup inputs conflict: {}",
                record.operation
            );
        }
        self.metadata.publish(
            &snapshot,
            vec![(
                identity(record.operation),
                serde_json::to_value(record).context("encoding setup")?,
            )],
            vec![],
        )?;
        Ok(())
    }
}
