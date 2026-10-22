use super::*;
use crate::{
    core::source::AcquireOptions,
    models::{
        index::lifecycle::Phase,
        setup::{Inputs, Outcome, State},
        store::Platform,
    },
    utils::{
        fs::atomic_write,
        lock::{LockGuard, LockMode},
    },
};
use std::{fs, path::Path, time::Duration};

#[derive(Debug, Clone)]
pub struct Options {
    pub built_shim: PathBuf,
    pub tag: String,
    pub operation: Option<Uuid>,
    pub migrate_unmanaged: bool,
    pub expected_sha256: Option<String>,
}

/// Explicit initialization is limited to absent/empty roots or reserved init recovery.
pub fn setup(root: &Path, input: &Options, acquire: &AcquireOptions) -> Result<Outcome> {
    validate_options(input)?;
    let initialized = root.join("root.json").try_exists()?;
    ensure!(
        !input.migrate_unmanaged || initialized,
        "migration requires an initialized store; preserve existing files or choose a separate root"
    );
    let store = if initialized {
        Store::open(root)?
    } else {
        match Store::create(root) {
            Ok(store) => store,
            // A concurrent initializer may have completed while create checked the root.
            Err(_) if root.join("root.json").try_exists()? => Store::open(root)?,
            Err(error) => return Err(error),
        }
    };
    store.setup(input, acquire)
}

fn validate_options(input: &Options) -> Result<()> {
    preparation::asset_name(&input.tag, &Platform::host())?;
    crate::core::shims::digest(&input.built_shim)?;
    ensure!(
        input.migrate_unmanaged == input.expected_sha256.is_some(),
        "migration requires both --migrate-unmanaged and --expected-sha256"
    );
    if input.migrate_unmanaged {
        ensure!(
            input.operation.is_some(),
            "migration requires an explicit operation ID"
        );
        let hash = input.expected_sha256.as_ref().expect("checked");
        ensure!(
            hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()),
            "expected SHA256 must contain 64 hexadecimal digits"
        );
    }
    Ok(())
}

impl Store {
    pub fn setup(&self, input: &Options, acquire: &AcquireOptions) -> Result<Outcome> {
        self.setup_admin()?;
        validate_options(input)?;
        let _setup = LockGuard::acquire(
            &beneath(&self.root, ".locks/setup")?,
            LockMode::Exclusive,
            Duration::from_secs(30),
        )?;
        let inputs = Inputs {
            store: self.metadata.authenticator().store(),
            root: self.root.clone(),
            tag: input.tag.clone(),
            platform: Platform::host(),
            shim_digest: crate::core::shims::digest(&input.built_shim)?,
            migrate_unmanaged: input.migrate_unmanaged,
            expected_sha256: input
                .expected_sha256
                .as_ref()
                .map(|v| v.to_ascii_lowercase()),
        };
        let records = self.setup_records()?;
        let existing = input
            .operation
            .and_then(|id| records.iter().find(|r| r.operation == id));
        if let Some(record) = existing {
            ensure!(
                record.inputs == inputs,
                "setup inputs conflict: {}",
                record.operation
            );
            if record.result.status.terminal() {
                let mut result = record.result.clone();
                if result.status == Phase::Complete {
                    let observed = self.setup_status()?;
                    ensure!(
                        observed.state == State::Ready
                            && observed.app == result.app
                            && observed.installed_version.as_deref() == Some(&input.tag),
                        "historical setup is no longer ready; inspect setup-status and use explicit maintenance"
                    );
                    result.changed = false;
                }
                return Ok(result);
            }
        }
        let mut record = if let Some(record) = existing {
            record.clone()
        } else {
            let status = self.setup_status()?;
            match status.state {
                State::Ready => {
                    ensure!(
                        status.installed_version.as_deref() == Some(&input.tag),
                        "installed release differs; use explicit self-update APP CLAIM --tag {}",
                        input.tag
                    );
                    let operation = records
                        .iter()
                        .find(|r| r.result.app == status.app)
                        .map(|r| r.operation)
                        .unwrap_or_else(Uuid::new_v4);
                    return Ok(Outcome {
                        schema_version: SCHEMA_VERSION,
                        root: self.root.clone(),
                        status: Phase::Complete,
                        changed: false,
                        launcher_path: status.launcher_path,
                        app: status.app,
                        claim: status.claim,
                        output: status.output,
                        installed_version: status.installed_version,
                        operation,
                        lifecycle_operation: None,
                        backup_paths: vec![],
                        diagnostics: vec![],
                        retry_shim: None,
                    });
                }
                State::Pending => {
                    if let Some(id) = status.operation {
                        let mut result = self.setup_record(id)?.result;
                        result.changed = false;
                        result
                            .diagnostics
                            .push(format!("retry setup explicitly with --operation {id}"));
                        return Ok(result);
                    }
                    anyhow::bail!("{}", status.diagnostics.join("; "));
                }
                State::Conflict => anyhow::bail!("{}", status.diagnostics.join("; ")),
                State::Unmanaged => ensure!(
                    input.migrate_unmanaged,
                    "unowned manager; use explicit verified migration with expected digest and operation ID"
                ),
                State::Absent => ensure!(
                    !input.migrate_unmanaged || self.preserved_migration(&inputs)?.is_some(),
                    "migration source is absent and no matching verified backup is recorded"
                ),
            }
            let mut record =
                self.prepare_setup_record(input.operation.unwrap_or_else(Uuid::new_v4), inputs)?;
            if record.inputs.migrate_unmanaged {
                record.migration = if status.state == State::Absent {
                    self.preserved_migration(&record.inputs)?
                } else {
                    Some(self.preflight_migration(&record)?)
                };
                if let Some(migration) = &record.migration
                    && migration.phase == crate::models::setup::MigrationPhase::Displaced
                {
                    record.result.backup_paths = vec![migration.backup.clone()];
                }
            }
            self.save_setup(&record)?;
            record
        };
        let result = self.run_setup(&mut record, input, acquire);
        if let Err(error) = result {
            if error.downcast_ref::<std::io::Error>().is_none() {
                record.result.status = Phase::Conflict;
            }
            record.result.diagnostics = vec![
                format!("{error:#}"),
                format!(
                    "retry with --operation {} and built shim {}",
                    record.operation,
                    record.shim.display()
                ),
            ];
        }
        self.save_setup(&record)?;
        Ok(record.result)
    }

    fn run_setup(
        &self,
        record: &mut Record,
        input: &Options,
        acquire: &AcquireOptions,
    ) -> Result<()> {
        if !record.shim.try_exists()? {
            atomic_write(&record.shim, |file| {
                std::io::copy(&mut fs::File::open(&input.built_shim)?, file)?;
                Ok(())
            })?;
        }
        ensure!(
            crate::core::shims::digest(&record.shim)? == record.inputs.shim_digest,
            "durable shim digest differs from setup input"
        );
        if record.inputs.migrate_unmanaged {
            self.migrate(record)?;
        }
        self.register_setup_source(record)?;
        self.register_policies(&record.profile.catalog)?;
        self.register_policies(std::slice::from_ref(&record.filter))?;
        let operation = self.bootstrap_with_operation(
            &record.request,
            &record.shim,
            acquire,
            Some(record.lifecycle_operation),
        )?;
        record.result.status = operation.status;
        record.result.changed |= operation.changed;
        record.result.app = Some(operation.app);
        record.result.claim = Some(operation.claim);
        record.result.output = operation.candidate;
        record.result.diagnostics = operation.pending;
        if record.inputs.migrate_unmanaged && operation.status != Phase::Complete {
            record.result.diagnostics.push("managed launcher is unavailable until publication completes; use the reported preserved backup and inspect the operation".into());
        }
        if operation.status == Phase::Complete {
            record.result.launcher_path =
                Some(self.root.join("bin").join(preparation::launcher_name()));
            record.result.installed_version = Some(record.inputs.tag.clone());
            record.result.retry_shim = None;
        }
        Ok(())
    }

    fn register_setup_source(&self, record: &Record) -> Result<()> {
        let _lock = self.lock()?;
        let source = crate::core::source::git::normalize(preparation::official_source())?;
        let id = record.request.source_uuid;
        let snapshot = self.metadata.snapshot()?;
        if snapshot.contains(crate::core::index::source_identity(id))? {
            ensure!(
                self.source(id)?.source == source,
                "reserved setup source differs"
            );
            return Ok(());
        }
        ensure!(
            self.find_source(&source)?.is_none(),
            "source was concurrently registered; start a new setup after inspecting this operation"
        );
        self.save_source(&crate::models::source::SourceRecord {
            uuid: id,
            source,
            policy_refs: Default::default(),
            policy_revision: 0,
            public_discovery: false,
            refresh: Default::default(),
            retrieval_freshness: None,
            discovery: Default::default(),
        })
    }
}
