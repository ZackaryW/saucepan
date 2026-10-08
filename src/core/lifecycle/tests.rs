use super::*;

#[test]
fn operation_identity_and_pending_work_are_isolated() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
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
    let reopened = Store::open(store.root()).unwrap();
    assert_eq!(
        reopened.reconcile(Some(id)).unwrap()[0].status,
        Phase::Preparing
    );
    assert!(reopened.register_app("unrelated".into()).is_ok());
    assert!(reopened.reconcile(Some(Uuid::new_v4())).is_err());
}

#[test]
fn lifecycle_journal_replays_records_retirement_and_receipt() {
    for published in [false, true] {
        for retired in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let store = Store::create(&tmp.path().join("root")).unwrap();
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
            let journal = Journal {
                operation: Some(op.clone()),
                retire: Some((app.uuid, op.operation)),
                ..Default::default()
            };
            index::write(&store.root.join(".staging/transaction.json"), &journal).unwrap();
            if retired {
                store
                    .retire_application_locked(app.uuid, op.operation)
                    .unwrap();
            }
            if published {
                store.save_operation(&op).unwrap();
            }
            let reopened = Store::open(store.root()).unwrap();
            assert!(reopened.records().unwrap().is_empty());
            assert_eq!(
                reopened.operation(op.operation).unwrap().status,
                Phase::Complete
            );
            assert!(!store.root.join(".staging/transaction.json").exists());
        }
    }
}

#[test]
fn removing_state_fences_mutations_and_legacy_defaults_are_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
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
}
