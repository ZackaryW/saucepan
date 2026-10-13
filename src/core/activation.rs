use crate::{
    core::{Store, index::Journal},
    models::{
        index::{Binding, Record},
        policy::Protection,
        store::Platform,
    },
    utils::{
        path::{beneath, plain_metadata, relative_path},
        process,
    },
};
use anyhow::{Result, ensure};
use std::{process::Command, time::Duration};
use uuid::Uuid;

pub use crate::models::index::lifecycle::Probe;

impl Store {
    /// Clear the active binding without releasing claims, leases, or cached content.
    /// A later activation may bind the application again.
    pub fn deactivate(&self, app: Uuid) -> Result<Option<Binding>> {
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        if let Some(binding) = &application.active {
            self.authorize_binding(
                app,
                binding.entry_uuid,
                crate::models::claim::policy::Action::Activate,
            )?;
        } else {
            self.authorize_application_action(app, crate::models::claim::policy::Action::Activate)?;
        }
        let previous = application.active.take();
        if previous.is_some() {
            application.bump()?;
            self.commit(Journal {
                records: vec![Record::Application(application)],
                stage: None,
                ..Default::default()
            })?;
        }
        Ok(previous)
    }

    pub fn activate(
        &self,
        app: Uuid,
        entry: Uuid,
        executable: &str,
        probe: &Probe,
    ) -> Result<Binding> {
        self.application(app)?;
        self.authorize_binding(app, entry, crate::models::claim::policy::Action::Activate)?;
        let _lease = self.validate_candidate(entry, executable, probe)?;
        let _lock = self.lock()?;
        self.authorize_binding(app, entry, crate::models::claim::policy::Action::Activate)?;
        self.metadata_path(&self.output(entry)?)?;
        let mut application = self.application(app)?;
        let binding = Binding {
            entry_uuid: entry,
            executable: executable.into(),
        };
        application.active = Some(binding.clone());
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            stage: None,
            ..Default::default()
        })?;
        Ok(binding)
    }
    pub(crate) fn validate_candidate(
        &self,
        entry: Uuid,
        executable: &str,
        probe: &Probe,
    ) -> Result<super::store::ResourceLease> {
        ensure!(
            probe.timeout_seconds > 0 && probe.timeout_seconds <= 120,
            "probe timeout must be 1..120 seconds"
        );
        ensure!(
            !probe.stdout_contains.is_empty(),
            "probe response contract is required"
        );
        relative_path(executable)?;
        let (path, mut lease, output) = {
            let _lock = self.lock()?;
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
            let lease = self.metadata_lease_locked(&output)?;
            let path = beneath(lease.directory(), executable)?;
            ensure!(
                plain_metadata(&path)?.is_file(),
                "candidate is not a regular file"
            );
            (path, lease, output)
        };
        lease.verify(self, &output)?;
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
        // A probe can modify its candidate. This distinct second scan is required.
        lease.verify(self, &output)?;
        Ok(lease)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        index::Settings,
        source::Source,
        store::{Request, Selector},
    };
    use std::fs;

    #[test]
    fn deactivate_missing_payload_preserves_concurrent_app_edits_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let input = tmp.path().join("data");
        fs::write(&input, b"data").unwrap();
        let source = store
            .register_source(Source::Local { path: input })
            .unwrap();
        let mut app = store.register_app("fixture".into()).unwrap();
        let request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policy_refs: Default::default(),
        };
        let claim = store
            .acquire_for(
                app.uuid,
                &"fixture".to_string().try_into().unwrap(),
                &request,
                &Default::default(),
            )
            .unwrap()
            .claim;
        app = store.application(app.uuid).unwrap();
        // An unavailable old binding must not prevent clearing the application.
        app.active = Some(Binding {
            entry_uuid: Uuid::new_v4(),
            executable: "missing".into(),
        });
        store
            .save_record(&Record::Application(app.clone()))
            .unwrap();
        let barrier = std::sync::Barrier::new(3);
        std::thread::scope(|scope| {
            let deactivate = scope.spawn(|| {
                barrier.wait();
                store.deactivate(app.uuid).unwrap().unwrap()
            });
            let settings = scope.spawn(|| {
                barrier.wait();
                store
                    .set_settings(
                        app.uuid,
                        Settings {
                            source_filter: vec![source.uuid],
                        },
                    )
                    .unwrap();
            });
            barrier.wait();
            assert_eq!(
                deactivate.join().unwrap().entry_uuid,
                app.active.as_ref().unwrap().entry_uuid
            );
            settings.join().unwrap();
        });
        let result = store.application(app.uuid).unwrap();
        assert!(result.active.is_none());
        assert_eq!(result.settings.source_filter, vec![source.uuid]);
        assert_eq!(result.name, app.name);
        assert_eq!(
            store.registry(result.uuid).unwrap().claims[&claim.uuid].entry_uuid,
            claim.entry_uuid
        );
        let path = store.root.join(format!("index/{}/index.json", app.uuid));
        // A no-op preserves signed bytes; even envelope formatting edits violate catalog membership.
        let mut bytes = fs::read(&path).unwrap();
        assert!(store.deactivate(app.uuid).unwrap().is_none());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        bytes.extend_from_slice(b" \n");
        fs::write(&path, &bytes).unwrap();
        assert!(store.deactivate(app.uuid).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(store.deactivate(Uuid::new_v4()).is_err());
        assert!(store.deactivate(claim.entry_uuid).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
