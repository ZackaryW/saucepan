use super::*;

#[test]
fn uninstall_fences_pending_maintenance_and_claim_publication() {
    use crate::models::{
        index::{Binding, lifecycle::ShimAction},
        source::Source,
        store::{Request, Selector},
    };
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let data = tmp.path().join("data");
    fs::write(&data, b"fixture").unwrap();
    let source = store
        .register_source(Source::Local { path: data.clone() })
        .unwrap();
    let request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    let entry = store.acquire(&request).unwrap();
    let mut app = store.register_app("remove".into()).unwrap();
    app.active = Some(Binding {
        entry_uuid: entry.entry.uuid,
        executable: "data".into(),
    });
    store
        .save_record(&Record::Application(app.clone()))
        .unwrap();
    store
        .install_shim(app.uuid, "fixture.exe", &data, false)
        .unwrap();
    let held =
        crate::core::shims::shim_lock(store.root(), "fixture.exe", LockMode::Shared).unwrap();
    let pending = store
        .maintain_shim(
            app.uuid,
            "fixture.exe",
            &data,
            ShimAction::Replace,
            false,
            None,
        )
        .unwrap();
    assert_eq!(pending.status, Phase::CommitIntent);
    let removed = store.uninstall(app.uuid, false, None).unwrap();
    assert_eq!(removed.status, Phase::CleanupPending);
    drop(held);
    assert_eq!(
        store.reconcile(Some(pending.operation)).unwrap()[0].status,
        Phase::Failed
    );
    assert_eq!(
        store.reconcile(Some(removed.operation)).unwrap()[0].status,
        Phase::Complete
    );
    assert!(
        store
            .claim_record(
                &crate::core::claim::operations::ClaimTarget {
                    projection: None,
                    expected_revision: None,
                    overrides: None,
                    scope: "fixture".to_string().try_into().unwrap(),
                    context: Default::default(),
                    app: app.uuid,
                    id: Uuid::new_v4(),
                    expected: None
                },
                &request,
                entry.entry.uuid
            )
            .is_err()
    );
    assert!(!store.root.join("bin/fixture.exe").exists());
    store.reclaim().unwrap();
    assert!(
        entry.directory.exists(),
        "surviving output policy retains content"
    );
}

#[test]
fn durable_candidate_survives_reclaim_and_invalid_candidate_releases_protection() {
    use crate::models::{
        index::lifecycle::{Installation, Probe},
        policy::{Eviction, Identical, Protection},
        source::Source,
        store::{Availability, Request, Selector},
    };
    // Interrupt before either file, after the sidecar, and after both files.
    // Also reject corrupt and missing candidates after partial publication.
    for phase in 0..5 {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_string_lossy().into_owned();
        let source = store
            .register_source(Source::Local { path: exe.clone() })
            .unwrap();
        let mut request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policies: Default::default(),
        };
        request.selection.bin = vec![name.clone()];
        request.policies.protection = Some(Protection::UseableApp);
        request.policies.identical = Identical::NewEntry;
        request.policies.eviction = Eviction::Latest { keep: 1 };
        let input = Installation {
            scope: "fixture".to_string().try_into().unwrap(),
            name: "recovery".into(),
            request,
            probe: Probe {
                args: vec!["--list".into()],
                stdout_contains: "test".into(),
                timeout_seconds: 10,
            },
            shim: "tool.exe".into(),
            saucepan: false,
        };
        let held =
            crate::core::shims::shim_lock(store.root(), &input.shim, LockMode::Shared).unwrap();
        let op = store
            .install_application(&input, &exe, &Default::default(), None)
            .unwrap();
        assert_eq!(op.status, Phase::CommitIntent, "{:?}", op.pending);
        let entry = op.candidate.unwrap();
        let next = store.acquire(&input.request).unwrap();
        assert_eq!(
            next.reclaim.deferred.get(&entry).map(String::as_str),
            Some("pending lifecycle commit intent")
        );
        drop(held);
        if phase >= 1 {
            fs::write(
                store.root.join("bin/tool.shim.json"),
                crate::core::shims::config_bytes(op.app, false).unwrap(),
            )
            .unwrap();
        }
        if phase >= 2 {
            fs::copy(&exe, store.root.join("bin/tool.exe")).unwrap();
        }
        assert!(store.application(op.app).is_err());
        if phase == 3 {
            fs::write(store.path(entry).unwrap().join(&name), b"corrupt").unwrap();
        } else if phase == 4 {
            fs::remove_file(store.path(entry).unwrap().join(&name)).unwrap();
        }
        let reopened = Store::open_fixture(store.root()).unwrap();
        let result = reopened.reconcile(Some(op.operation)).unwrap().remove(0);
        assert_eq!(
            result.status,
            if phase >= 3 {
                Phase::Failed
            } else {
                Phase::Complete
            },
            "{:?}",
            result.pending
        );
        if phase >= 3 {
            assert!(reopened.application(op.app).is_err());
            assert!(!store.root.join("bin/tool.exe").exists());
            assert!(!store.root.join("bin/tool.shim.json").exists());
            reopened.reclaim().unwrap();
            assert_eq!(
                reopened.output(entry).unwrap().state,
                Availability::Reclaimed
            );
        } else {
            assert_eq!(
                reopened
                    .application(op.app)
                    .unwrap()
                    .active
                    .unwrap()
                    .entry_uuid,
                entry
            );
            assert_eq!(reopened.registry(op.app).unwrap().claims.len(), 1);
            assert_eq!(reopened.operation(op.operation).unwrap().claim, op.claim);
        }
    }
}

#[test]
fn operation_identity_and_pending_work_are_isolated() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let app = store.register_app("fixture".into()).unwrap();
    let id = Uuid::new_v4();
    let action = Action::Uninstall {
        leave_unmanaged_shims: false,
    };
    let (op, guard) = store
        .begin_operation(id, Some(app.uuid), action.clone())
        .unwrap();
    assert!(
        LockGuard::acquire(
            &store.root.join(format!(".locks/operation-{id}")),
            LockMode::Exclusive,
            Duration::ZERO
        )
        .is_err()
    );
    drop(guard);
    let (same, guard) = store.begin_operation(id, Some(app.uuid), action).unwrap();
    assert_eq!(same.claim, op.claim);
    drop(guard);
    assert!(
        store
            .begin_operation(
                id,
                Some(app.uuid),
                Action::Uninstall {
                    leave_unmanaged_shims: true
                }
            )
            .is_err()
    );
    let reopened = Store::open_fixture(store.root()).unwrap();
    assert_eq!(
        reopened.reconcile(Some(id)).unwrap()[0].status,
        Phase::Preparing
    );
    assert!(reopened.register_app("unrelated".into()).is_ok());
    assert!(reopened.reconcile(Some(Uuid::new_v4())).is_err());
}

#[test]
fn lifecycle_journal_replays_registry_retirement_and_receipt() {
    for boundary in 0..5 {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let mut app = store.register_app("remove".into()).unwrap();
        let (mut op, guard) = store
            .begin_operation(
                Uuid::new_v4(),
                Some(app.uuid),
                Action::Uninstall {
                    leave_unmanaged_shims: false,
                },
            )
            .unwrap();
        drop(guard);
        app.state = AppState::Removing;
        app.bump().unwrap();
        store
            .save_record(&Record::Application(app.clone()))
            .unwrap();
        op.status = Phase::Complete;
        op.changed = true;
        store.interrupt_commit(
            Journal {
                operation: Some(op.clone()),
                retire: Some((app.uuid, op.operation)),
                ..Default::default()
            },
            boundary,
        );
        let reopened = Store::open_fixture(store.root()).unwrap();
        assert!(reopened.records().unwrap().is_empty());
        assert!(reopened.registry(app.uuid).is_err());
        assert_eq!(
            reopened.operation(op.operation).unwrap().status,
            Phase::Complete
        );
        assert!(
            !store
                .root
                .join(".staging/metadata-transaction.json")
                .exists()
        );
    }
}

#[test]
fn lifecycle_journal_couples_application_registry_and_receipt_at_each_boundary() {
    for boundary in 0..5 {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let mut app = store.register_app("transition".into()).unwrap();
        let (mut op, guard) = store
            .begin_operation(
                Uuid::new_v4(),
                Some(app.uuid),
                Action::Uninstall {
                    leave_unmanaged_shims: false,
                },
            )
            .unwrap();
        drop(guard);
        app.state = AppState::Removing;
        app.bump().unwrap();
        op.status = Phase::CleanupPending;
        op.changed = true;
        let record = Record::Application(app.clone());
        store.interrupt_commit(
            Journal {
                records: vec![record.clone()],
                operation: Some(op.clone()),
                ..Default::default()
            },
            boundary,
        );
        let reopened = Store::open_fixture(store.root()).unwrap();
        assert!(reopened.application(app.uuid).is_err());
        assert_eq!(
            serde_json::to_value(reopened.record(app.uuid).unwrap()).unwrap(),
            serde_json::to_value(record).unwrap()
        );
        assert_eq!(
            reopened.registry(app.uuid).unwrap().registry.revision,
            app.revision
        );
        assert_eq!(
            reopened.operation(op.operation).unwrap().status,
            Phase::CleanupPending
        );
        assert!(reopened.register_app("unrelated".into()).is_ok());
    }
}

#[test]
fn removing_state_fences_mutations_and_legacy_defaults_are_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let mut app = store.register_app("state".into()).unwrap();
    let mut json = serde_json::to_value(&app).unwrap();
    for field in ["revision", "state", "shims"] {
        json.as_object_mut().unwrap().remove(field);
    }
    let old: crate::models::index::Application = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(old.state, AppState::Ready);
    assert_eq!(old.revision, 0);
    json["unknown"] = true.into();
    assert!(serde_json::from_value::<crate::models::index::Application>(json).is_err());
    store.deactivate(app.uuid).unwrap();
    store.set_settings(app.uuid, Default::default()).unwrap();
    assert_eq!(store.application(app.uuid).unwrap().revision, 0);
    app.state = AppState::Removing;
    store
        .save_record(&Record::Application(app.clone()))
        .unwrap();
    assert!(store.deactivate(app.uuid).is_err());
    assert!(store.set_settings(app.uuid, Default::default()).is_err());
    assert!(store.release_claim(app.uuid, Uuid::new_v4()).is_err());
    assert!(
        store
            .set_claim_context(app.uuid, Uuid::new_v4(), &Default::default(), app.revision)
            .is_err()
    );
}
