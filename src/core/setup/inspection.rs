use super::*;
use crate::models::{
    index::Record as IndexRecord,
    setup::{State, Status},
    source::Resolved,
};
use std::{fs, path::Path};

fn empty_status(root: PathBuf) -> Status {
    Status {
        schema_version: SCHEMA_VERSION,
        root,
        state: State::Absent,
        launcher_path: None,
        app: None,
        claim: None,
        output: None,
        installed_version: None,
        operation: None,
        diagnostics: vec![],
    }
}

// Canonicalize the nearest existing ancestor without creating the missing suffix.
fn absent_root(root: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(root)?;
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    while !ancestor.try_exists()? {
        suffix.push(
            ancestor
                .file_name()
                .context("invalid setup root")?
                .to_owned(),
        );
        ancestor = ancestor
            .parent()
            .context("setup root has no existing ancestor")?;
    }
    let mut root = ancestor.canonicalize()?;
    for name in suffix.into_iter().rev() {
        root.push(name);
    }
    Ok(root)
}

/// Inspection uses metadata-only opening: pending metadata is not reconciled here.
pub fn inspect(root: &Path) -> Result<Status> {
    if !root.try_exists()? {
        return Ok(empty_status(absent_root(root)?));
    }
    Store::open_metadata(root, &crate::core::claim::security::NativeKeyring)?.setup_status()
}

impl Store {
    pub fn setup_status(&self) -> Result<Status> {
        self.setup_admin()?;
        let mut inspection = self.clone();
        inspection.metadata_only = true;
        inspection.setup_status_read()
    }

    fn setup_status_read(&self) -> Result<Status> {
        let _lock = self.lock()?;
        let mut result = empty_status(self.root.clone());
        let name = super::preparation::launcher_name();
        let launcher = beneath(&self.root, &format!("bin/{name}"))?;
        let sidecar = beneath(
            &self.root,
            &format!("bin/{}", crate::core::shims::sidecar(name)?),
        )?;
        let records = self.setup_records()?;
        let pending: Vec<_> = records
            .iter()
            .filter(|r| !r.result.status.terminal())
            .collect();
        if !pending.is_empty() {
            result.state = if pending.len() == 1 {
                State::Pending
            } else {
                State::Conflict
            };
            result.operation = Some(pending[0].operation);
            result
                .diagnostics
                .push("inspect the recorded setup operation; retry it explicitly".into());
            return Ok(result);
        }
        let mut candidates = Vec::new();
        for record in self.records()? {
            if let IndexRecord::Application(app) = record {
                let registry = self.registry(app.uuid)?;
                let claims: Vec<_> = registry
                    .claims
                    .values()
                    .filter(|claim| claim.scope.as_str() == "saucepan/self")
                    .cloned()
                    .collect();
                if !claims.is_empty() || app.shims.iter().any(|s| s.saucepan) {
                    candidates.push((app, claims));
                }
            }
        }
        if candidates.is_empty() {
            // Include low-level bootstrap operations which have not published an app yet.
            if let Some(op) = self.operations(None)?.iter().find(|op| {
                !op.status.terminal()
                    && matches!(
                        op.action,
                        crate::models::index::lifecycle::Action::Install { saucepan: true, .. }
                    )
            }) {
                result.state = State::Pending;
                result.diagnostics.push(format!(
                    "pending lifecycle operation {}; reconcile or retry explicitly",
                    op.operation
                ));
            } else if fs::symlink_metadata(&sidecar).is_ok() {
                result.state = State::Conflict;
                result
                    .diagnostics
                    .push("unowned launcher sidecar; inspect without replacement".into());
            } else if fs::symlink_metadata(&launcher).is_ok() {
                result.state = State::Unmanaged;
                result.launcher_path = Some(launcher);
                result
                    .diagnostics
                    .push("raw manager requires explicit verified migration".into());
            }
            return Ok(result);
        }
        result.state = State::Conflict;
        if candidates.len() != 1 || candidates[0].1.len() != 1 {
            result
                .diagnostics
                .push("ambiguous self application/claim identity".into());
            return Ok(result);
        }
        let (app, claims) = &candidates[0];
        let claim = &claims[0];
        result.app = Some(app.uuid);
        result.claim = Some(claim.uuid);
        result.output = Some(claim.entry_uuid);
        result.launcher_path = Some(launcher.clone());
        let check = (|| -> Result<String> {
            ensure!(
                app.state == crate::models::index::AppState::Ready,
                "self application is removing"
            );
            let binding = app
                .active
                .as_ref()
                .context("self application has no active binding")?;
            ensure!(
                binding.entry_uuid == claim.entry_uuid && binding.executable == name,
                "self binding differs from its claim"
            );
            let owned = app
                .shims
                .iter()
                .find(|s| s.name == name && s.saucepan)
                .context("self launcher is not owned")?;
            ensure!(
                owned.sidecar == crate::core::shims::sidecar(name)?
                    && crate::core::shims::digest(&launcher)? == owned.digest
                    && crate::core::shims::digest(&sidecar)? == owned.config_digest,
                "owned launcher changed; use shim maintenance"
            );
            let config: crate::core::launch::ShimConfig =
                serde_json::from_slice(&fs::read(&sidecar)?)?;
            ensure!(
                config.app == app.uuid && config.saucepan,
                "launcher configuration differs from application"
            );
            let output = self.output(claim.entry_uuid)?;
            ensure!(
                output.request == claim.request && output.request.selection.bin == [name],
                "self output differs from claim request"
            );
            let directory = self.verified_path(&output)?;
            ensure!(
                crate::utils::path::plain_metadata(&beneath(&directory, name)?)?.is_file(),
                "active manager is missing"
            );
            match output.resolved {
                Resolved::Release { tag, .. } => Ok(tag),
                _ => anyhow::bail!("self output is not a release"),
            }
        })();
        match check {
            Ok(tag) => {
                result.state = State::Ready;
                result.installed_version = Some(tag);
            }
            Err(error) => result.diagnostics.push(error.to_string()),
        }
        Ok(result)
    }
}
