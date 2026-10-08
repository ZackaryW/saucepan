//! Explicit ownership of a launcher and its configuration pair.
use super::{Store, launch::ShimConfig};
use crate::{
    models::index::{
        OwnedShim, Record,
        lifecycle::{Action, Operation, Phase, ShimAction, ShimChange},
    },
    utils::{
        fs::atomic_write,
        lock::{LockGuard, LockMode},
        path::{beneath, plain_metadata, relative_path},
        tree,
    },
};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

pub(crate) fn sidecar(name: &str) -> Result<String> {
    let path = relative_path(name)?;
    ensure!(
        path.components().count() == 1,
        "shim name must be one component"
    );
    let config = path
        .with_extension("shim.json")
        .to_string_lossy()
        .into_owned();
    ensure!(
        !name.to_lowercase().ends_with(".shim.json")
            && name.to_lowercase() != config.to_lowercase(),
        "shim name aliases configuration"
    );
    Ok(config)
}
pub(crate) fn shim_lock(root: &Path, name: &str, mode: LockMode) -> Result<LockGuard> {
    sidecar(name)?;
    let key = super::store::json_digest(&name.to_lowercase())?;
    Ok(LockGuard::acquire(
        &beneath(root, &format!(".locks/shim-{key}"))?,
        mode,
        Duration::ZERO,
    )?)
}
pub(crate) fn digest(path: &Path) -> Result<String> {
    ensure!(
        plain_metadata(path)?.is_file(),
        "not a regular file: {}",
        path.display()
    );
    Ok(tree::digest(fs::File::open(path)?)?)
}
pub(crate) fn config_bytes(app: Uuid, saucepan: bool) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(&ShimConfig { app, saucepan })?)
}
fn names(shim: &OwnedShim) -> [String; 2] {
    [shim.name.to_lowercase(), shim.sidecar.to_lowercase()]
}

impl Store {
    pub(crate) fn remove_owned_shim_locked(&self, owned: &OwnedShim) -> Result<()> {
        for (name, expected) in [
            (&owned.name, &owned.digest),
            (&owned.sidecar, &owned.config_digest),
        ] {
            let path = beneath(&self.root, &format!("bin/{name}"))?;
            if path.exists() {
                ensure!(&digest(&path)? == expected, "owned file changed: {name}");
            }
        }
        for name in [&owned.name, &owned.sidecar] {
            let path = beneath(&self.root, &format!("bin/{name}"))?;
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
    pub(crate) fn reserve_shim_locked(
        &self,
        op: &Operation,
        desired: &OwnedShim,
        previous: Option<&OwnedShim>,
    ) -> Result<()> {
        let wanted = names(desired);
        for record in self.records()? {
            if let Record::Application(app) = record {
                for owned in app.shims {
                    if app.uuid == op.app && previous == Some(&owned) {
                        continue;
                    }
                    ensure!(
                        !names(&owned).iter().any(|p| wanted.contains(p)),
                        "shim name already owned"
                    );
                }
            }
        }
        for other in self.operations_locked()? {
            if other.operation == op.operation || other.status.terminal() {
                continue;
            }
            for change in other.shims {
                ensure!(
                    !names(&change.desired).iter().any(|p| wanted.contains(p)),
                    "shim name reserved by operation {}",
                    other.operation
                );
            }
        }
        // Case-insensitive checks also catch aliases on case-sensitive hosts.
        for entry in fs::read_dir(beneath(&self.root, "bin")?)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if wanted.contains(&name.to_lowercase()) {
                ensure!(
                    previous.is_some_and(|old| old.name == name || old.sidecar == name),
                    "unowned shim destination occupied: {name}"
                );
            }
        }
        for name in [&desired.name, &desired.sidecar] {
            beneath(&self.root, &format!("bin/{name}"))?;
        }
        Ok(())
    }

    pub fn maintain_shim(
        &self,
        app: Uuid,
        name: &str,
        built: &Path,
        action: ShimAction,
        saucepan: bool,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        let config = sidecar(name)?;
        let binary_digest = digest(built)?;
        let (mut op, _guard) = self.begin_operation(
            operation.unwrap_or_else(Uuid::new_v4),
            Some(app),
            Action::Shim {
                action,
                name: name.into(),
                digest: binary_digest.clone(),
                saucepan,
            },
        )?;
        if op.status.terminal() {
            return Ok(op);
        }
        if op.status == Phase::Preparing {
            let prepared = (|| -> Result<()> {
                let bytes = config_bytes(app, saucepan)?;
                let desired = OwnedShim {
                    name: name.into(),
                    sidecar: config,
                    digest: binary_digest,
                    config_digest: tree::digest(bytes.as_slice())?,
                    saucepan,
                };
                {
                    let _lock = self.lock()?;
                    let application = self.application(app)?;
                    ensure!(
                        application.active.is_some(),
                        "activate before installing a shim"
                    );
                    let previous = application
                        .shims
                        .iter()
                        .find(|s| s.name.to_lowercase() == name.to_lowercase())
                        .cloned();
                    match action {
                        ShimAction::Install => {
                            ensure!(previous.is_none(), "shim already owned; use replace-shim");
                            self.reserve_shim_locked(&op, &desired, None)?;
                        }
                        ShimAction::Replace => {
                            let old = previous.as_ref().context("shim is not owned")?;
                            ensure!(
                                old.name == name
                                    && old.saucepan == saucepan
                                    && old.config_digest == desired.config_digest,
                                "replacement cannot repoint a shim"
                            );
                            self.reserve_shim_locked(&op, &desired, Some(old))?;
                        }
                        ShimAction::Adopt => {
                            ensure!(previous.is_none(), "shim already owned");
                            self.reserve_shim_locked(&op, &desired, Some(&desired))?;
                            ensure!(
                                digest(&beneath(&self.root, &format!("bin/{name}"))?)?
                                    == desired.digest,
                                "adoption binary mismatch"
                            );
                            ensure!(
                                digest(&beneath(&self.root, &format!("bin/{}", desired.sidecar))?)?
                                    == desired.config_digest,
                                "adoption configuration mismatch"
                            );
                        }
                    }
                    op.shims = vec![ShimChange { previous, desired }];
                    self.save_operation(&op)?;
                }
                let staged = beneath(&self.operation_dir(op.operation)?, "launcher")?;
                atomic_write(&staged, |file| {
                    std::io::copy(&mut fs::File::open(built)?, file)?;
                    Ok(())
                })?;
                ensure!(
                    digest(&staged)? == op.shims[0].desired.digest,
                    "built shim changed during staging"
                );
                let _lock = self.lock()?;
                self.application(app)?;
                op.status = Phase::CommitIntent;
                op.pending.clear();
                self.save_operation(&op)
            })();
            if let Err(error) = prepared {
                let _lock = self.lock()?;
                op.status = Phase::Failed;
                op.pending = vec![format!("{error:#}")];
                self.save_operation(&op)?;
                return Ok(op);
            }
        }
        self.resume_shim(&mut op)?;
        Ok(op)
    }

    pub(crate) fn publish_shim_locked(&self, op: &Operation, change: &ShimChange) -> Result<()> {
        let desired = &change.desired;
        let binary = beneath(&self.root, &format!("bin/{}", desired.name))?;
        let sidecar = beneath(&self.root, &format!("bin/{}", desired.sidecar))?;
        let staged = beneath(&self.operation_dir(op.operation)?, "launcher")?;
        ensure!(
            digest(&staged)? == desired.digest,
            "staged launcher integrity mismatch"
        );
        // A replay recognizes both the previous and the already-published identity.
        for (path, wanted, old) in [
            (
                &binary,
                &desired.digest,
                change.previous.as_ref().map(|s| &s.digest),
            ),
            (
                &sidecar,
                &desired.config_digest,
                change.previous.as_ref().map(|s| &s.config_digest),
            ),
        ] {
            if path.exists() {
                let found = digest(path)?;
                ensure!(
                    &found == wanted || old == Some(&found),
                    "owned file changed: {}",
                    path.display()
                );
            }
        }
        let bytes = config_bytes(op.app, desired.saucepan)?;
        ensure!(
            tree::digest(bytes.as_slice())? == desired.config_digest,
            "sidecar identity mismatch"
        );
        if !sidecar.exists() {
            atomic_write(&sidecar, |f| {
                use std::io::Write;
                f.write_all(&bytes)
            })?;
        }
        if !binary.exists() || digest(&binary)? != desired.digest {
            atomic_write(&binary, |file| {
                std::io::copy(&mut fs::File::open(&staged)?, file)?;
                Ok(())
            })?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }

    pub(crate) fn resume_shim(&self, op: &mut Operation) -> Result<()> {
        let change = op
            .shims
            .first()
            .context("shim operation lacks ownership intent")?
            .clone();
        let result = (|| -> Result<()> {
            let _shim = shim_lock(&self.root, &change.desired.name, LockMode::Exclusive)?;
            let _lock = self.lock()?;
            let mut app = self.application(op.app)?;
            let previous = app.shims.iter().find(|s| s.name == change.desired.name);
            ensure!(
                previous == change.previous.as_ref() || previous == Some(&change.desired),
                "shim ownership changed during maintenance"
            );
            if matches!(
                op.action,
                Action::Shim {
                    action: ShimAction::Adopt,
                    ..
                }
            ) {
                ensure!(
                    digest(&beneath(
                        &self.root,
                        &format!("bin/{}", change.desired.name)
                    )?)? == change.desired.digest,
                    "adoption binary mismatch"
                );
                ensure!(
                    digest(&beneath(
                        &self.root,
                        &format!("bin/{}", change.desired.sidecar)
                    )?)? == change.desired.config_digest,
                    "adoption configuration mismatch"
                );
            } else {
                self.publish_shim_locked(op, &change)?;
            }
            app.shims.retain(|s| s.name != change.desired.name);
            app.shims.push(change.desired.clone());
            app.bump()?;
            op.status = Phase::Complete;
            op.changed = true;
            op.pending.clear();
            self.commit_operation(op, vec![Record::Application(app)])
        })();
        if let Err(error) = result {
            let _lock = self.lock()?;
            *op = self.operation(op.operation)?;
            if op.status == Phase::Complete {
                return Ok(());
            }
            op.pending = vec![format!("{}: {error:#}", change.desired.name)];
            self.save_operation(op)?;
        }
        Ok(())
    }

    pub fn install_shim(
        &self,
        app: Uuid,
        name: &str,
        built: &Path,
        saucepan: bool,
    ) -> Result<PathBuf> {
        let result = self.maintain_shim(app, name, built, ShimAction::Install, saucepan, None)?;
        ensure!(
            result.status == Phase::Complete,
            "shim operation {}: {:?}",
            result.operation,
            result.pending
        );
        Ok(self.root.join("bin").join(name))
    }
}

#[cfg(test)]
mod tests;
