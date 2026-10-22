use super::*;
use crate::{
    core::shims,
    models::{
        index::{Record as IndexRecord, lifecycle::Action},
        setup::{Migration, MigrationPhase},
    },
    utils::{lock::LockMode, path::plain_metadata, process, transfer::copy_bounded},
};
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    process::Command,
    time::Duration,
};

fn unlinked(path: &Path) -> Result<File> {
    ensure!(
        plain_metadata(path)?.is_file(),
        "migration requires a plain regular file: {}",
        path.display()
    );
    let file = File::open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            file.metadata()?.nlink() == 1,
            "migration rejects hard links"
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };
        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // The borrowed file owns this handle throughout the native query.
        unsafe {
            GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information)?;
        }
        ensure!(
            information.nNumberOfLinks == 1,
            "migration rejects hard links"
        );
    }
    Ok(file)
}

fn verify(path: &Path, digest: &str) -> Result<()> {
    ensure!(
        crate::utils::tree::digest(unlinked(path)?)? == digest,
        "migration digest changed: {}",
        path.display()
    );
    Ok(())
}

impl Store {
    pub(super) fn preserved_migration(
        &self,
        inputs: &crate::models::setup::Inputs,
    ) -> Result<Option<Migration>> {
        let mut candidates = self
            .setup_records()?
            .into_iter()
            .filter(|r| {
                matches!(
                    r.result.status,
                    crate::models::index::lifecycle::Phase::Failed
                        | crate::models::index::lifecycle::Phase::Conflict
                )
            })
            .filter_map(|r| r.migration)
            .filter(|m| {
                m.phase == MigrationPhase::Displaced
                    && Some(&m.digest) == inputs.expected_sha256.as_ref()
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.backup.cmp(&b.backup));
        candidates.dedup_by(|a, b| a.backup == b.backup);
        ensure!(
            candidates.len() <= 1,
            "multiple preserved backups match; inspect before retrying"
        );
        if let Some(migration) = candidates.first() {
            verify(&migration.backup, &migration.digest)?;
        }
        Ok(candidates.pop())
    }

    fn migration_destination(&self, lifecycle: Uuid) -> Result<PathBuf> {
        let name = preparation::launcher_name();
        let sidecar = shims::sidecar(name)?;
        for entry in fs::read_dir(beneath(&self.root, "bin")?)? {
            let entry = entry?;
            let actual = entry.file_name().to_string_lossy().into_owned();
            ensure!(
                actual.to_lowercase() != sidecar.to_lowercase(),
                "migration refuses an occupied sidecar"
            );
            ensure!(
                actual.to_lowercase() != name.to_lowercase() || actual == name,
                "migration refuses a case-alias launcher"
            );
        }
        for record in self.records()? {
            if let IndexRecord::Application(app) = record {
                ensure!(
                    !app.shims.iter().any(|s| s.name.eq_ignore_ascii_case(name)
                        || s.sidecar.eq_ignore_ascii_case(&sidecar)),
                    "launcher is already owned"
                );
            }
        }
        for op in self.operations_locked()? {
            if op.operation != lifecycle && !op.status.terminal() {
                ensure!(
                    !matches!(&op.action, Action::Install { shim, .. } if shim.eq_ignore_ascii_case(name))
                        && !op
                            .shims
                            .iter()
                            .any(|s| s.desired.name.eq_ignore_ascii_case(name)),
                    "launcher reserved by operation {}",
                    op.operation
                );
            }
        }
        Ok(beneath(&self.root, &format!("bin/{name}"))?)
    }

    pub(super) fn preflight_migration(&self, record: &Record) -> Result<Migration> {
        let _shim = shims::shim_lock(
            &self.root,
            preparation::launcher_name(),
            LockMode::Exclusive,
        )?;
        let source = {
            let _lock = self.lock()?;
            self.migration_destination(record.lifecycle_operation)?
        };
        let digest = record
            .inputs
            .expected_sha256
            .clone()
            .context("migration digest missing")?;
        ensure!(
            std::env::current_exe()?.canonicalize()? != source.canonicalize()?,
            "run migration from an externally staged manager"
        );
        verify(&source, &digest)?;
        let mut command = Command::new(&source);
        command.arg("--version");
        let probe = process::run(command, Duration::from_secs(10), 65536)?;
        ensure!(
            probe.status.success()
                && !probe.truncated
                && String::from_utf8_lossy(&probe.stdout).starts_with("saucepan "),
            "migration candidate is not a supported raw manager"
        );
        verify(&source, &digest)?;
        Ok(Migration {
            phase: MigrationPhase::Intent,
            source,
            backup: self
                .setup_dir(record.operation)?
                .join(preparation::launcher_name()),
            digest,
        })
    }

    pub(super) fn migrate(&self, record: &mut Record) -> Result<()> {
        let _shim = shims::shim_lock(
            &self.root,
            preparation::launcher_name(),
            LockMode::Exclusive,
        )?;
        let migration = record
            .migration
            .clone()
            .context("migration intent missing")?;
        ensure!(
            migration.digest
                == record
                    .inputs
                    .expected_sha256
                    .as_deref()
                    .context("missing migration digest")?,
            "migration consent differs"
        );
        if migration.phase == MigrationPhase::Displaced {
            verify(&migration.backup, &migration.digest)?;
            // A linked committed install may already have published its launcher.
            if migration.source.try_exists()? {
                let op = self.operation(record.lifecycle_operation)?;
                ensure!(
                    matches!(
                        op.status,
                        crate::models::index::lifecycle::Phase::CommitIntent
                            | crate::models::index::lifecycle::Phase::Complete
                    ),
                    "destination occupied after displacement"
                );
                ensure!(
                    op.shims
                        .iter()
                        .any(|s| shims::digest(&migration.source).ok().as_ref()
                            == Some(&s.desired.digest)),
                    "destination changed after displacement"
                );
            }
            return Ok(());
        }
        {
            let _lock = self.lock()?;
            ensure!(
                self.migration_destination(record.lifecycle_operation)? == migration.source,
                "migration destination changed"
            );
        }
        if migration.phase == MigrationPhase::Intent {
            verify(&migration.source, &migration.digest)?;
            if !migration.backup.try_exists()? {
                let mut backup = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&migration.backup)?;
                let source = unlinked(&migration.source)?;
                let permissions = source.metadata()?.permissions();
                let copied = copy_bounded(source, &mut backup, 512 * 1024 * 1024, true)?;
                ensure!(
                    copied.sha256.as_deref() == Some(&migration.digest),
                    "source changed while copying backup"
                );
                backup.set_permissions(permissions)?;
                backup.sync_all()?;
            }
            // Never overwrite a partial or changed backup on retry.
            verify(&migration.backup, &migration.digest)?;
            record.migration.as_mut().expect("intent").phase = MigrationPhase::BackedUp;
            record.result.backup_paths = vec![migration.backup.clone()];
            self.save_setup(record)?;
        }
        verify(&migration.backup, &migration.digest)?;
        let _lock = self.lock()?;
        self.migration_destination(record.lifecycle_operation)?;
        if migration.source.try_exists()? {
            verify(&migration.source, &migration.digest)?;
            fs::remove_file(&migration.source).with_context(|| "raw launcher remains in place; close processes using it and retry this operation")?;
        }
        record.migration.as_mut().expect("intent").phase = MigrationPhase::Displaced;
        record.result.changed = true;
        record.result.diagnostics = vec!["raw launcher displaced; preserved backup is available until managed publication completes".into()];
        self.save_setup(record)?;
        Ok(())
    }
}
