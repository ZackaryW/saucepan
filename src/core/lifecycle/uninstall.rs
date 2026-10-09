use super::*;
use crate::{core::shims, models::index::lifecycle::ShimChange};

impl Store {
    pub fn uninstall(
        &self,
        app: Uuid,
        leave_unmanaged_shims: bool,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        let id = match operation {
            Some(id) => id,
            None => self
                .operations(Some(app))?
                .into_iter()
                .find(|op| {
                    matches!(op.action, Action::Uninstall { .. })
                        && !matches!(op.status, Phase::Failed | Phase::Conflict)
                })
                .map(|op| op.operation)
                .unwrap_or_else(Uuid::new_v4),
        };
        let (mut op, _guard) = self.begin_operation(
            id,
            Some(app),
            Action::Uninstall {
                leave_unmanaged_shims,
            },
        )?;
        if op.status.terminal() {
            return Ok(op);
        }
        if op.status == Phase::Preparing {
            let result = (|| -> Result<()> {
                let _lock = self.lock()?;
                let mut application = self.application(app)?;
                self.authorize_application_action(
                    app,
                    crate::models::claim::policy::Action::Remove,
                )?;
                let mut changes: Vec<_> = application
                    .shims
                    .iter()
                    .map(|owned| ShimChange {
                        previous: None,
                        desired: owned.clone(),
                    })
                    .collect();
                // A publication may have stopped before recording ownership in the app.
                // Its durable intent identifies the files; removing state fences its writer.
                for other in self.operations_locked()? {
                    if other.app != app || other.operation == id || other.status.terminal() {
                        continue;
                    }
                    for change in other.shims {
                        if let Some(owned) = changes
                            .iter_mut()
                            .find(|s| s.desired.name == change.desired.name)
                        {
                            *owned = change;
                        } else {
                            changes.push(change);
                        }
                    }
                }
                let mut unmanaged = Vec::new();
                for entry in fs::read_dir(beneath(&self.root, "bin")?)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !name.ends_with(".shim.json")
                        || changes.iter().any(|s| s.desired.sidecar == name)
                    {
                        continue;
                    }
                    let path = beneath(&self.root, &format!("bin/{name}"))?;
                    if !plain_metadata(&path)?.is_file() {
                        continue;
                    }
                    if let Ok(config) = index::read::<crate::core::launch::ShimConfig>(&path)
                        && config.app == app
                    {
                        unmanaged.push(format!("bin/{name} and its unmanaged launcher"));
                    }
                }
                ensure!(
                    unmanaged.is_empty() || leave_unmanaged_shims,
                    "unregistered shims remain; adopt them or use --leave-unmanaged-shims: {}",
                    unmanaged.join(", ")
                );
                op.leftovers = unmanaged;
                op.shims = changes;
                op.status = Phase::CleanupPending;
                op.reason = Some(crate::error::Error::new(
                    crate::error::ErrorCode::IoError,
                    "application cleanup pending",
                ));
                op.changed = true;
                op.pending.clear();
                application.state = AppState::Removing;
                application.active = None;
                let mut registry = self.registry(app)?;
                registry.claims.clear();
                op.desired_registry = Some(registry);
                application.bump()?;
                self.commit_operation(&op, vec![Record::Application(application)])
            })();
            if let Err(error) = result {
                let _lock = self.lock()?;
                let persisted = self.operation(id)?;
                if persisted.status != Phase::Preparing {
                    op = persisted;
                } else {
                    op.status = Phase::Failed;
                    op.changed = false;
                    op.reason = Some(crate::error::Error::from_anyhow(&error));
                    op.pending = vec![op.reason.as_ref().unwrap().message.clone()];
                    self.save_operation(&op)?;
                    return Ok(op);
                }
            }
        }
        self.resume_uninstall(&mut op)?;
        Ok(op)
    }

    pub(crate) fn resume_uninstall(&self, op: &mut Operation) -> Result<()> {
        let mut pending = Vec::new();
        for change in &op.shims {
            let result = (|| -> Result<()> {
                let _shim =
                    shims::shim_lock(&self.root, &change.desired.name, LockMode::Exclusive)?;
                let _lock = self.lock()?;
                // A replacement may have published either the old or the new digest.
                for (name, desired, old) in [
                    (
                        &change.desired.name,
                        &change.desired.digest,
                        change.previous.as_ref().map(|s| &s.digest),
                    ),
                    (
                        &change.desired.sidecar,
                        &change.desired.config_digest,
                        change.previous.as_ref().map(|s| &s.config_digest),
                    ),
                ] {
                    let path = beneath(&self.root, &format!("bin/{name}"))?;
                    if path.exists() {
                        let found = shims::digest(&path)?;
                        ensure!(
                            &found == desired || old == Some(&found),
                            "owned file changed: {name}"
                        );
                    }
                }
                for name in [&change.desired.name, &change.desired.sidecar] {
                    let path = beneath(&self.root, &format!("bin/{name}"))?;
                    if path.exists() {
                        fs::remove_file(path)?;
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                op.reason = Some(crate::error::Error::from_anyhow(&error));
                pending.push(format!("{}: cleanup pending", change.desired.name));
            }
        }
        let _lock = self.lock()?;
        op.pending = pending;
        if !op.pending.is_empty() {
            return self.save_operation(op);
        }
        // Retire live app and registry membership in the same signed generation.
        if self
            .metadata
            .snapshot()?
            .contains(crate::core::claim::registry::registry_identity(op.app))?
        {
            self.commit(Journal {
                operation: Some(op.clone()),
                retire: Some((op.app, op.operation)),
                ..Default::default()
            })?;
        }
        let cleanup = (|| -> Result<()> {
            // Only remove empty directories; preserve and report foreign files.
            for relative in [format!("index/{}", op.app), format!("claims/{}", op.app)] {
                let path = beneath(&self.root, &relative)?;
                if path.exists() {
                    fs::remove_dir(path)?;
                }
            }
            Ok(())
        })();
        match cleanup {
            Ok(()) => {
                op.status = Phase::Complete;
                op.reason = None;
                op.pending.clear();
            }
            Err(error) => {
                op.reason = Some(crate::error::Error::from_anyhow(&error));
                op.pending = vec!["retired application cleanup pending".into()];
            }
        }
        self.save_operation(op)
    }
}
