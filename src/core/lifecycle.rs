//! Durable application operations. File maintenance never runs in Store::open.
use super::{
    Store,
    index::{self, Journal},
};
use crate::{
    models::index::{
        AppState, Record,
        lifecycle::{Action, Operation, Phase},
    },
    utils::{
        lock::{LockGuard, LockMode},
        path::{beneath, plain_metadata},
    },
};
use anyhow::{Context, Result, ensure};
use std::{fs, path::PathBuf, time::Duration};
use uuid::Uuid;

impl Store {
    pub(crate) fn operation_dir(&self, id: Uuid) -> Result<PathBuf> {
        Ok(beneath(&self.root, &format!(".staging/lifecycle/{id}"))?)
    }
    pub(crate) fn operation_lock(&self, id: Uuid) -> Result<LockGuard> {
        Ok(LockGuard::acquire(
            &beneath(&self.root, &format!(".locks/operation-{id}"))?,
            LockMode::Exclusive,
            Duration::from_secs(30),
        )?)
    }
    pub fn operation(&self, id: Uuid) -> Result<Operation> {
        let op: Operation = index::read(&beneath(
            &self.root,
            &format!(".staging/lifecycle/{id}/index.json"),
        )?)?;
        ensure!(op.operation == id, "operation identity mismatch");
        Ok(op)
    }
    pub(crate) fn save_operation(&self, op: &Operation) -> Result<()> {
        let dir = self.operation_dir(op.operation)?;
        fs::create_dir_all(&dir)?;
        index::write(&dir.join("index.json"), op)
    }
    pub(crate) fn operations_locked(&self) -> Result<Vec<Operation>> {
        let path = beneath(&self.root, ".staging/lifecycle")?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut operations = Vec::new();
        for item in fs::read_dir(path)? {
            let item = item?;
            let id = Uuid::parse_str(&item.file_name().to_string_lossy())
                .context("invalid operation directory")?;
            // Directory creation may have stopped before the first atomic record write.
            if self.operation_dir(id)?.join("index.json").exists() {
                operations.push(self.operation(id)?);
            }
        }
        operations.sort_by_key(|op| op.operation);
        Ok(operations)
    }
    pub fn operations(&self, app: Option<Uuid>) -> Result<Vec<Operation>> {
        let _lock = self.lock()?;
        Ok(self
            .operations_locked()?
            .into_iter()
            .filter(|op| app.is_none_or(|app| op.app == app))
            .collect())
    }
    /// Reserve a stable operation identity. Retain the returned guard during execution.
    pub fn begin_operation(
        &self,
        id: Uuid,
        app: Option<Uuid>,
        action: Action,
    ) -> Result<(Operation, LockGuard)> {
        let guard = self.operation_lock(id)?;
        let _lock = self.lock()?;
        let fingerprint = super::store::json_digest(&(app, &action))?;
        let record_path = self.operation_dir(id)?.join("index.json");
        if record_path.exists() {
            let op = self.operation(id)?;
            ensure!(
                op.fingerprint == fingerprint,
                "operation inputs conflict: {id}"
            );
            return Ok((op, guard));
        }
        let revision = app
            .map(|id| self.application(id).map(|app| app.revision))
            .transpose()?;
        let claim = match &action {
            Action::Update { claim, .. } => *claim,
            _ => Uuid::new_v4(),
        };
        let op = Operation {
            operation: id,
            app: app.unwrap_or_else(Uuid::new_v4),
            action,
            fingerprint,
            status: Phase::Preparing,
            changed: false,
            expected_revision: revision,
            claim,
            candidate: None,
            desired: None,
            shims: Vec::new(),
            pending: Vec::new(),
            leftovers: Vec::new(),
        };
        self.commit_operation(&op, Vec::new())?;
        Ok((op, guard))
    }
    pub(crate) fn retire_application_locked(&self, app: Uuid, operation: Uuid) -> Result<()> {
        let source = beneath(&self.root, &format!("index/{app}"))?;
        let dest = beneath(&self.operation_dir(operation)?, "removed-app")?;
        ensure!(
            !(source.exists() && dest.exists()),
            "conflicting retired application"
        );
        if source.exists() {
            ensure!(
                plain_metadata(&source)?.is_dir(),
                "invalid application directory"
            );
            let contents: Vec<_> = fs::read_dir(&source)?.collect::<std::io::Result<_>>()?;
            ensure!(
                contents.len() == 1 && contents[0].file_name() == "index.json",
                "unknown files in application directory"
            );
            match self.record(app)? {
                Record::Application(a) => ensure!(
                    a.state == AppState::Removing && a.active.is_none() && a.claims.is_empty(),
                    "application not ready for retirement"
                ),
                _ => anyhow::bail!("not an application"),
            }
            fs::rename(source, dest)?;
        }
        Ok(())
    }
    pub(crate) fn commit_operation(&self, op: &Operation, records: Vec<Record>) -> Result<()> {
        self.commit(Journal {
            records,
            operation: Some(op.clone()),
            ..Default::default()
        })
    }
    /// Resume durable metadata intent. Preparation requires the original command.
    pub fn reconcile(&self, id: Option<Uuid>) -> Result<Vec<Operation>> {
        let ids = match id {
            Some(id) => vec![id],
            None => self
                .operations(None)?
                .into_iter()
                .filter(|o| !o.status.terminal())
                .map(|o| o.operation)
                .collect(),
        };
        ids.into_iter()
            .map(|id| {
                let _guard = self.operation_lock(id)?;
                let mut op = {
                    let _lock = self.lock()?;
                    self.operation(id)?
                };
                if !op.status.terminal() {
                    if matches!(op.action, Action::Shim { .. }) && op.status != Phase::Preparing {
                        self.resume_shim(&mut op)?;
                    } else if matches!(op.action, Action::Install { .. } | Action::Update { .. })
                        && op.status != Phase::Preparing
                    {
                        self.resume_application(&mut op)?;
                    } else if matches!(op.action, Action::Uninstall { .. })
                        && op.status != Phase::Preparing
                    {
                        self.resume_uninstall(&mut op)?;
                    } else {
                        let _lock = self.lock()?;
                        op.pending =
                            vec!["retry the original command to finish preparation".into()];
                        self.save_operation(&op)?;
                    }
                }
                Ok(op)
            })
            .collect()
    }
}

mod managed;
#[cfg(test)]
mod tests;
mod uninstall;
