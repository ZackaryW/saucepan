use crate::{
    core::{Store, index::Journal, launch::ShimConfig},
    models::{
        index::{Binding, Record},
        policy::Protection,
        store::Platform,
    },
    utils::{
        fs::atomic_write,
        path::{beneath, plain_metadata, relative_path},
        process,
    },
};
use anyhow::{Context, Result, ensure};
use std::{fs, path::Path, process::Command, time::Duration};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    pub args: Vec<String>,
    pub stdout_contains: String,
    pub timeout_seconds: u64,
}
impl Store {
    pub fn activate(
        &self,
        app: Uuid,
        entry: Uuid,
        executable: &str,
        probe: &Probe,
    ) -> Result<Binding> {
        ensure!(
            probe.timeout_seconds > 0 && probe.timeout_seconds <= 120,
            "probe timeout must be 1..120 seconds"
        );
        ensure!(
            !probe.stdout_contains.is_empty(),
            "probe response contract is required"
        );
        relative_path(executable)?;
        let (path, _lease) = {
            let _lock = self.lock()?;
            self.application(app)?;
            let output = self.output(entry)?;
            ensure!(
                output.request.policies.protection == Some(Protection::UseableApp),
                "activation requires useable_app policy"
            );
            ensure!(
                output
                    .request
                    .selection
                    .platform
                    .as_ref()
                    .is_none_or(|p| *p == Platform::host()),
                "candidate platform mismatch"
            );
            ensure!(
                output.request.selection.bin.iter().any(|b| b == executable),
                "executable was not declared"
            );
            let root = self.verified_path(&output)?;
            let path = beneath(&root, executable)?;
            ensure!(
                plain_metadata(&path)?.is_file(),
                "candidate is not a regular file"
            );
            (path, self.lease_locked(entry)?)
        };
        let mut command = Command::new(&path);
        command.args(&probe.args);
        let result = process::run(command, Duration::from_secs(probe.timeout_seconds), 65536)?;
        ensure!(
            result.status.success()
                && !result.truncated
                && String::from_utf8_lossy(&result.stdout).contains(&probe.stdout_contains),
            "usability probe failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let _lock = self.lock()?;
        self.verified_path(&self.output(entry)?)?;
        let mut application = self.application(app)?;
        let binding = Binding {
            entry_uuid: entry,
            executable: executable.into(),
        };
        application.active = Some(binding.clone());
        self.commit(Journal {
            records: vec![Record::Application(application)],
            stage: None,
        })?;
        Ok(binding)
    }
    pub fn install_shim(
        &self,
        app: Uuid,
        name: &str,
        built_shim: &Path,
        saucepan: bool,
    ) -> Result<std::path::PathBuf> {
        relative_path(name)?;
        ensure!(!name.contains('/'), "shim name must be one component");
        ensure!(plain_metadata(built_shim)?.is_file(), "built shim missing");
        let _lock = self.lock()?;
        self.application(app)?
            .active
            .context("activate before installing a shim")?;
        let dest = beneath(&self.root, &format!("bin/{name}"))?;
        ensure!(
            !dest.exists(),
            "shim already installed; replacement is a separate maintenance operation"
        );
        super::index::write(
            &dest.with_extension("shim.json"),
            &ShimConfig { app, saucepan },
        )?;
        atomic_write(&dest, |file| {
            std::io::copy(&mut fs::File::open(built_shim)?, file)?;
            Ok(())
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
        }
        Ok(dest)
    }
}
