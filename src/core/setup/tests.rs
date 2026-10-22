use super::*;
use crate::models::{setup::Inputs, store::Platform};

fn fixture(store: &Store) -> Record {
    store
        .prepare_setup_record(
            Uuid::new_v4(),
            Inputs {
                store: store.metadata.authenticator().store(),
                root: store.root.clone(),
                tag: "v0.11.0".into(),
                platform: Platform::host(),
                shim_digest: "a".repeat(64),
                migrate_unmanaged: false,
                expected_sha256: None,
            },
        )
        .unwrap()
}

#[test]
fn records_survive_reopen_in_both_integrity_modes_without_new_identities() {
    for protected in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("store");
        let store = if protected {
            Store::create_fixture(&root)
        } else {
            Store::create(&root)
        }
        .unwrap();
        let record = fixture(&store);
        store.save_setup(&record).unwrap();
        let before = std::fs::read(root.join("root.json")).unwrap();
        drop(store);
        let reopened = if protected {
            Store::open_fixture(&root)
        } else {
            Store::open(&root)
        }
        .unwrap();
        let restored = reopened.setup_record(record.operation).unwrap();
        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(reopened.setup_records().unwrap().len(), 1);
        assert_eq!(before, std::fs::read(root.join("root.json")).unwrap());
        assert_eq!(
            reopened.integrity_mode() == crate::models::claim::security::IntegrityMode::Keyring,
            protected
        );
    }
}

#[test]
fn changed_preparation_and_tampered_receipts_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::create(&temp.path().join("store")).unwrap();
    let record = fixture(&store);
    store.save_setup(&record).unwrap();
    let path = store
        .setup_dir(record.operation)
        .unwrap()
        .join("index.json");
    let bytes = std::fs::read(&path).unwrap();
    let mut changed = record.clone();
    changed.inputs.tag = "v99.0.0".into();
    changed.fingerprint = crate::core::store::json_digest(&changed.inputs).unwrap();
    assert!(store.save_setup(&changed).is_err());
    changed = record.clone();
    changed.lifecycle_operation = Uuid::new_v4();
    assert!(store.save_setup(&changed).is_err());
    assert_eq!(bytes, std::fs::read(&path).unwrap());
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace("v0.11.0", "v99.0.0");
    std::fs::write(&path, text).unwrap();
    assert!(store.setup_record(record.operation).is_err());
    assert!(Store::open(store.root()).is_err());
}

#[test]
fn inspection_does_not_create_missing_root_and_preserves_raw_occupants() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing/nested");
    let result = inspection::inspect(&missing).unwrap();
    assert_eq!(result.state, crate::models::setup::State::Absent);
    assert!(!temp.path().join("missing").exists());
    let store = Store::create(&temp.path().join("store")).unwrap();
    let launcher = store.root.join("bin").join(preparation::launcher_name());
    std::fs::write(&launcher, b"raw manager").unwrap();
    let before = std::fs::read(store.root.join("root.json")).unwrap();
    assert_eq!(
        inspection::inspect(store.root()).unwrap().state,
        crate::models::setup::State::Unmanaged
    );
    assert_eq!(before, std::fs::read(store.root.join("root.json")).unwrap());
    assert_eq!(std::fs::read(launcher).unwrap(), b"raw manager");
}

#[test]
fn migration_recovery_never_overwrites_a_backup_or_removes_changed_source() {
    use crate::models::setup::{Migration, MigrationPhase};
    let temp = tempfile::tempdir().unwrap();
    let store = Store::create(&temp.path().join("store")).unwrap();
    let mut record = fixture(&store);
    let source = store.root.join("bin").join(preparation::launcher_name());
    std::fs::write(&source, b"original manager").unwrap();
    let digest = crate::core::shims::digest(&source).unwrap();
    record.inputs.migrate_unmanaged = true;
    record.inputs.expected_sha256 = Some(digest.clone());
    record.fingerprint = crate::core::store::json_digest(&record.inputs).unwrap();
    let backup = store
        .setup_dir(record.operation)
        .unwrap()
        .join(preparation::launcher_name());
    record.migration = Some(Migration {
        phase: MigrationPhase::Intent,
        source: source.clone(),
        backup: backup.clone(),
        digest,
    });
    store.save_setup(&record).unwrap();
    std::fs::write(&backup, b"partial backup").unwrap();
    assert!(store.migrate(&mut record).is_err());
    assert_eq!(std::fs::read(&backup).unwrap(), b"partial backup");
    assert_eq!(std::fs::read(&source).unwrap(), b"original manager");

    // Model the persisted boundary after a verified backup but before displacement.
    std::fs::write(&backup, b"original manager").unwrap();
    record.migration.as_mut().unwrap().phase = MigrationPhase::BackedUp;
    record.result.backup_paths = vec![backup.clone()];
    store.save_setup(&record).unwrap();
    std::fs::write(&source, b"another writer").unwrap();
    assert!(store.migrate(&mut record).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"another writer");
    assert_eq!(std::fs::read(&backup).unwrap(), b"original manager");

    // Model a crash after displacement but before the phase receipt was published.
    std::fs::remove_file(&source).unwrap();
    store.migrate(&mut record).unwrap();
    assert_eq!(
        record.migration.as_ref().unwrap().phase,
        MigrationPhase::Displaced
    );
    assert_eq!(
        store
            .setup_record(record.operation)
            .unwrap()
            .result
            .backup_paths,
        [backup]
    );
    assert!(!source.exists());
}

#[test]
fn historical_completion_does_not_override_current_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::create(&temp.path().join("store")).unwrap();
    let mut record = fixture(&store);
    record.result.status = crate::models::index::lifecycle::Phase::Complete;
    store.save_setup(&record).unwrap();
    assert_eq!(
        store.setup_status().unwrap().state,
        crate::models::setup::State::Absent
    );
}
