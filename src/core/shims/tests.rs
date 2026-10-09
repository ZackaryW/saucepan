use super::*;
use crate::models::index::Binding;

fn setup() -> (tempfile::TempDir, Store, Uuid, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let mut app = store.register_app("shim fixture".into()).unwrap();
    app.active = Some(Binding {
        entry_uuid: Uuid::new_v4(),
        executable: "unused".into(),
    });
    store
        .save_record(&Record::Application(app.clone()))
        .unwrap();
    let built = tmp.path().join("built");
    fs::write(&built, b"fixture launcher bytes").unwrap();
    (tmp, store, app.uuid, built)
}

#[test]
fn shim_publication_recovers_each_file_boundary_and_retry_is_idempotent() {
    for phase in 0..3 {
        let (_tmp, store, app, built) = setup();
        let held = shim_lock(store.root(), "tool.exe", LockMode::Shared).unwrap();
        let op = store
            .maintain_shim(app, "tool.exe", &built, ShimAction::Install, false, None)
            .unwrap();
        assert_eq!(op.status, Phase::CommitIntent);
        assert!(!op.pending.is_empty());
        drop(held);
        if phase >= 1 {
            fs::write(
                store.root.join("bin/tool.shim.json"),
                config_bytes(app, false).unwrap(),
            )
            .unwrap();
        }
        if phase >= 2 {
            fs::copy(&built, store.root.join("bin/tool.exe")).unwrap();
        }
        let reopened = Store::open_fixture(store.root()).unwrap();
        assert!(reopened.register_app("unrelated".into()).is_ok());
        let done = reopened.reconcile(Some(op.operation)).unwrap().remove(0);
        assert_eq!(done.status, Phase::Complete, "{:?}", done.pending);
        assert_eq!(reopened.application(app).unwrap().shims.len(), 1);
        let revision = reopened.application(app).unwrap().revision;
        let repeated = reopened
            .maintain_shim(
                app,
                "tool.exe",
                &built,
                ShimAction::Install,
                false,
                Some(op.operation),
            )
            .unwrap();
        assert_eq!(repeated.status, Phase::Complete);
        assert_eq!(reopened.application(app).unwrap().revision, revision);
    }
}

#[test]
fn shim_collisions_and_replacement_conflicts_preserve_existing_files() {
    let (_tmp, store, app, built) = setup();
    fs::write(store.root.join("bin/tool.shim.json"), b"unowned").unwrap();
    let failed = store
        .maintain_shim(app, "tool.exe", &built, ShimAction::Install, false, None)
        .unwrap();
    assert_eq!(failed.status, Phase::Failed);
    assert_eq!(
        fs::read(store.root.join("bin/tool.shim.json")).unwrap(),
        b"unowned"
    );
    fs::remove_file(store.root.join("bin/tool.shim.json")).unwrap();
    store.install_shim(app, "tool.exe", &built, false).unwrap();
    for alias in ["TOOL.EXE", "tool", "tool.shim.json"] {
        assert!(store.install_shim(app, alias, &built, false).is_err());
    }
    let old = store.application(app).unwrap();
    fs::write(store.root.join("bin/tool.exe"), b"externally changed").unwrap();
    let pending = store
        .maintain_shim(app, "tool.exe", &built, ShimAction::Replace, false, None)
        .unwrap();
    assert_eq!(pending.status, Phase::CommitIntent);
    assert_eq!(
        fs::read(store.root.join("bin/tool.exe")).unwrap(),
        b"externally changed"
    );
    assert_eq!(store.application(app).unwrap().revision, old.revision);
    fs::copy(&built, store.root.join("bin/tool.exe")).unwrap();
    assert_eq!(
        store.reconcile(Some(pending.operation)).unwrap()[0].status,
        Phase::Complete
    );
}

#[test]
fn adoption_is_explicit_and_missing_owned_pair_can_be_restored() {
    let (_tmp, store, app, built) = setup();
    fs::copy(&built, store.root.join("bin/tool.exe")).unwrap();
    fs::write(
        store.root.join("bin/tool.shim.json"),
        config_bytes(app, false).unwrap(),
    )
    .unwrap();
    let adopted = store
        .maintain_shim(app, "tool.exe", &built, ShimAction::Adopt, false, None)
        .unwrap();
    assert_eq!(adopted.status, Phase::Complete);
    fs::remove_file(store.root.join("bin/tool.exe")).unwrap();
    fs::remove_file(store.root.join("bin/tool.shim.json")).unwrap();
    let restored = store
        .maintain_shim(app, "tool.exe", &built, ShimAction::Replace, false, None)
        .unwrap();
    assert_eq!(restored.status, Phase::Complete);
    assert_eq!(
        fs::read(store.root.join("bin/tool.exe")).unwrap(),
        fs::read(&built).unwrap()
    );
}

#[test]
fn concurrent_names_have_one_owner() {
    let (_tmp, store, first, built) = setup();
    let mut second = store.register_app("second".into()).unwrap();
    second.active = store.application(first).unwrap().active;
    store
        .save_record(&Record::Application(second.clone()))
        .unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            barrier.wait();
            store
                .maintain_shim(first, "tool.exe", &built, ShimAction::Install, false, None)
                .unwrap()
        });
        let two = scope.spawn(|| {
            barrier.wait();
            store
                .maintain_shim(
                    second.uuid,
                    "tool.exe",
                    &built,
                    ShimAction::Install,
                    false,
                    None,
                )
                .unwrap()
        });
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(
            results
                .iter()
                .filter(|r| r.status == Phase::Complete)
                .count(),
            1
        );
        assert_eq!(
            results.iter().filter(|r| r.status == Phase::Failed).count(),
            1
        );
    });
}
