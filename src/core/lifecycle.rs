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
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let reference = snapshot
            .catalog
            .records
            .get(&format!(".staging/lifecycle/{id}/index.json"))
            .context("operation missing")?;
        let op: Operation = snapshot.read(self.metadata.authenticator(), reference.identity)?;
        ensure!(
            op.operation == id && index::operation_identity(&op) == reference.identity,
            "operation identity mismatch"
        );
        Ok(op)
    }
    pub(crate) fn save_operation(&self, op: &Operation) -> Result<()> {
        let snapshot = self.metadata.snapshot()?;
        self.metadata.publish(
            &snapshot,
            vec![(index::operation_identity(op), serde_json::to_value(op)?)],
            vec![],
        )?;
        Ok(())
    }
    pub(crate) fn operations_locked(&self) -> Result<Vec<Operation>> {
        let snapshot = self.metadata.snapshot()?;
        let mut operations = Vec::new();
        for reference in snapshot
            .catalog
            .records
            .values()
            .filter(|r| r.identity.kind == crate::core::claim::security::Kind::Operation)
        {
            let op: Operation = snapshot.read(self.metadata.authenticator(), reference.identity)?;
            ensure!(
                index::operation_identity(&op) == reference.identity,
                "operation identity mismatch"
            );
            operations.push(op);
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
        if let Some(app) = app {
            if self.session.is_some()
                || self
                    .metadata
                    .snapshot()?
                    .contains(crate::core::claim::registry::registry_identity(app))?
            {
                use crate::models::claim::policy::Action as Permission;
                match &action {
                    Action::Update { claim, .. } => {
                        let registry = self.registry(app)?;
                        self.check_session(&registry)?;
                        let claim =
                            registry
                                .claims
                                .get(claim)
                                .context(crate::error::Error::new(
                                    crate::error::ErrorCode::UnknownClaim,
                                    "claim missing",
                                ))?;
                        self.authorize_claim(&registry, claim, Permission::Update)?;
                        self.authorize_claim(&registry, claim, Permission::Activate)?;
                    }
                    Action::Uninstall { .. } => {
                        self.authorize_application_action(app, Permission::Remove)?
                    }
                    Action::Shim { .. } => {
                        self.authorize_application_action(app, Permission::Activate)?
                    }
                    Action::Install { .. } => anyhow::bail!("invalid existing-app installation"),
                }
            }
        } else {
            ensure!(
                self.session.is_none(),
                crate::error::Error::new(
                    crate::error::ErrorCode::AccessDenied,
                    "installation requires administration"
                )
            );
        }
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
        let source = match &action {
            Action::Install { request, .. } => Some(request.source_uuid),
            Action::Update { claim, .. } => Some(
                self.registry(app.context("update app missing")?)?
                    .claims
                    .get(claim)
                    .context("update claim missing")?
                    .request
                    .source_uuid,
            ),
            _ => None,
        };
        let source_policy = source
            .map(|source_uuid| -> Result<_> {
                Ok(crate::models::index::lifecycle::SourcePolicyObservation {
                    source_uuid,
                    revision: self.source(source_uuid)?.policy_revision,
                })
            })
            .transpose()?;
        let op = Operation {
            source_policy,
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
            desired_registry: None,
            shims: Vec::new(),
            pending: Vec::new(),
            reason: None,
            leftovers: Vec::new(),
        };
        self.commit_operation(&op, Vec::new())?;
        Ok((op, guard))
    }
    pub(crate) fn commit_operation(&self, op: &Operation, records: Vec<Record>) -> Result<()> {
        let registries = op.desired_registry.as_ref().filter(|r| records.iter().any(|record| matches!(record, Record::Application(app) if app.uuid == r.registry.app))).cloned().into_iter().collect();
        self.commit(Journal {
            records,
            registries,
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
                        op.reason = Some(crate::error::Error::new(
                            crate::error::ErrorCode::InvalidRequest,
                            "original command must complete preparation",
                        ));
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
