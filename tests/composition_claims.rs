mod native_root;
mod policy_fixture;
mod support;
use saucepan::{
    core::{
        claim::session::ApplicationRegistration,
        retrieval::{RegisteredRetriever, RetrieveOptions},
        store::Acquirer,
    },
    error::{Error, ErrorCode},
    models::{
        claim::{EnsureRequest, Scope, policy::AccessPolicy},
        index::Record,
        policy::UpdatePolicy,
        retrieval::{RetrieveRequest, SourceInput},
        source::Source,
        store::{AcquisitionRequest, Destination, Selector},
    },
};
use std::{collections::BTreeMap, fs, path::Path};
fn scope(value: &str) -> Scope {
    value.to_owned().try_into().unwrap()
}
fn snapshot(path: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, path: &Path, output: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, output);
            } else {
                output.insert(
                    path.strip_prefix(root).unwrap().to_string_lossy().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut output = BTreeMap::new();
    walk(path, path, &mut output);
    output
}
fn at(time: u64) -> RetrieveOptions {
    RetrieveOptions {
        test_now: Some(time),
        ..Default::default()
    }
}

#[test]
fn unclaimed_operations_obey_scope_source_and_action_and_keep_independent_check_state() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, b"one").unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: file.clone() })
        .unwrap();
    let app = store
        .register_application(
            &ApplicationRegistration {
                registration_key: scope("app"),
                name: "app".into(),
                credentials_required: false,
                policy_refs: policy_fixture::register(
                    &store,
                    AccessPolicy {
                        sources: Some([source.uuid].into()),
                        scopes: Some([scope("a"), scope("b")].into()),
                        ..Default::default()
                    },
                    Default::default(),
                )
                .unwrap(),
            },
            None,
        )
        .unwrap()
        .application
        .uuid;
    let session = store.session(app, None).unwrap();
    let mut retrieval = RetrieveRequest {
        source: SourceInput::Registered {
            source_uuid: source.uuid,
        },
        selector: Selector::Local,
        asset: Default::default(),
        policy_refs: Default::default(),
        destination: None,
        expected_sha256: None,
    };
    let acquisition = AcquisitionRequest {
        policy_refs: policy_fixture::definitions(
            &store,
            vec![serde_json::json!({"type":"update_policy","mode":"max_age","seconds":60})],
        )
        .unwrap(),
        ..Default::default()
    };
    assert!(session.retrieve(&retrieval, None, &at(100)).is_err());
    let mut denied_source = retrieval.clone();
    denied_source.source = SourceInput::Registered {
        source_uuid: uuid::Uuid::new_v4(),
    };
    assert!(
        session
            .retrieve(&denied_source, Some(&scope("a")), &at(100))
            .is_err()
    );
    denied_source.source = SourceInput::Inline {
        source: Source::Local { path: file.clone() },
    };
    policy_fixture::replace(
        &store,
        &mut denied_source.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"temporary"}),
    )
    .unwrap();
    assert!(
        session
            .retrieve(&denied_source, Some(&scope("a")), &at(100))
            .is_err()
    );
    assert!(
        session
            .acquire_composed(&retrieval, &acquisition, Some(&scope("denied")), &at(100))
            .is_err()
    );
    assert_eq!(store.records().unwrap().len(), 1);
    let first = session
        .acquire_composed(&retrieval, &acquisition, Some(&scope("a")), &at(100))
        .unwrap();
    fs::write(file, b"two").unwrap();
    let second = session
        .acquire_composed(&retrieval, &acquisition, Some(&scope("b")), &at(120))
        .unwrap();
    let unchanged = session
        .acquire_composed(&retrieval, &acquisition, Some(&scope("a")), &at(130))
        .unwrap();
    assert_eq!(first.path(), unchanged.path());
    assert_ne!(first.path(), second.path());
    assert!(session.claims().unwrap().is_empty());
    assert_eq!(
        store
            .records()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, Record::Acquisition(_)))
            .count(),
        2
    );
    policy_fixture::replace(
        &store,
        &mut retrieval.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"temporary"}),
    )
    .unwrap();
    let before = snapshot(store.root());
    let reader = RegisteredRetriever::open(store.root())
        .unwrap()
        .session(app, None)
        .unwrap();
    assert!(reader.retrieve(&retrieval, None, &at(140)).is_err());
    let fresh = reader
        .retrieve(&retrieval, Some(&scope("a")), &at(140))
        .unwrap();
    let copied = Acquirer::acquire(
        &fresh,
        &AcquisitionRequest {
            destination: Destination::Temporary {},
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(copied.path().join("input")).unwrap(), b"two");
    assert_eq!(snapshot(store.root()), before);
}

#[test]
fn automatic_ensure_preserves_claim_identity_context_and_scope_and_does_not_bind() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, b"one").unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: file.clone() })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap().uuid;
    let session = store.session(app, None).unwrap();
    let mut request = EnsureRequest {
        scope: scope("a"),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: [("project".into(), "a".into())].into(),
        policy_refs: policy_fixture::requested(
            &store,
            policy_fixture::Overrides {
                update_policy: Some(UpdatePolicy::MaxAge { seconds: 60 }),
                ..Default::default()
            },
        )
        .unwrap(),
    };
    let options = |time| saucepan::core::source::AcquireOptions {
        test_now: Some(time),
        ..Default::default()
    };
    let first = session.ensure(&request, &options(100)).unwrap();
    fs::write(file, b"two").unwrap();
    assert_eq!(
        session
            .ensure(&request, &options(159))
            .unwrap()
            .claim
            .entry_uuid,
        first.claim.entry_uuid
    );
    let updated = session.ensure(&request, &options(160)).unwrap();
    assert_eq!(updated.claim.uuid, first.claim.uuid);
    assert_ne!(updated.claim.entry_uuid, first.claim.entry_uuid);
    assert_eq!(updated.claim.context, request.context);
    assert!(store.application(app).unwrap().active.is_none());
    request.scope = scope("b");
    let other = session.ensure(&request, &options(161)).unwrap();
    assert_ne!(other.claim.uuid, first.claim.uuid);
    assert_eq!(session.claims().unwrap().len(), 2);
}

#[test]
fn registered_fresh_reader_reports_pending_recovery_without_replaying_it() {
    let temp = tempfile::tempdir().unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    fs::write(
        store.root().join(".staging/metadata-transaction.json"),
        b"pending test fixture",
    )
    .unwrap();
    let before = snapshot(store.root());
    let error = match RegisteredRetriever::open(store.root()) {
        Ok(_) => panic!("must require reconciliation"),
        Err(error) => error,
    };
    assert_eq!(Error::from_anyhow(&error).code, ErrorCode::RecoveryRequired);
    assert_eq!(snapshot(store.root()), before);
}

#[test]
fn automatic_updates_require_registered_allowances_and_update_permission() {
    use policy_fixture::Registration;
    use saucepan::models::claim::policy::Action;
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, b"one").unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let register = |name: &str, policies| {
        store
            .register_application(
                &ApplicationRegistration {
                    registration_key: scope(name),
                    name: name.into(),
                    credentials_required: false,
                    policy_refs: policy_fixture::register(
                        &store,
                        AccessPolicy {
                            actions: Some([Action::Acquire, Action::Read].into()),
                            ..Default::default()
                        },
                        policies,
                    )
                    .unwrap(),
                },
                None,
            )
            .unwrap()
            .application
            .uuid
    };
    let denied = register(
        "policy-denied",
        Registration {
            allow_automatic_updates: false,
            ..Default::default()
        },
    );
    let request = EnsureRequest {
        scope: scope("a"),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: Default::default(),
        policy_refs: policy_fixture::requested(
            &store,
            policy_fixture::Overrides {
                update_policy: Some(UpdatePolicy::MaxAge { seconds: 60 }),
                ..Default::default()
            },
        )
        .unwrap(),
    };
    assert!(
        store
            .session(denied, None)
            .unwrap()
            .ensure(&request, &Default::default())
            .is_err()
    );
    let app = register("acquire-only", Registration::default());
    let session = store.session(app, None).unwrap();
    let options = |time| saucepan::core::source::AcquireOptions {
        test_now: Some(time),
        ..Default::default()
    };
    let first = session.ensure(&request, &options(100)).unwrap();
    assert_eq!(
        session.ensure(&request, &options(159)).unwrap().claim.uuid,
        first.claim.uuid
    );
    let error = session.ensure(&request, &options(160)).unwrap_err();
    assert_eq!(Error::from_anyhow(&error).code, ErrorCode::AccessDenied);
}

#[test]
fn disappearing_covered_folder_does_not_advance_automatic_owner() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    fs::create_dir_all(input.join("child")).unwrap();
    fs::write(input.join("child/file"), b"one").unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local {
            path: input.clone(),
        })
        .unwrap();
    let app = store.register_app("app".into()).unwrap().uuid;
    let session = store.session(app, None).unwrap();
    let mut request = EnsureRequest {
        scope: scope("a"),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: Default::default(),
        policy_refs: policy_fixture::requested(
            &store,
            policy_fixture::Overrides {
                update_policy: Some(UpdatePolicy::Always {}),
                ..Default::default()
            },
        )
        .unwrap(),
    };
    let first = session.ensure(&request, &Default::default()).unwrap();
    let before = serde_json::to_value(
        store
            .records()
            .unwrap()
            .into_iter()
            .filter(|record| matches!(record, Record::Acquisition(_)))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    fs::rename(input.join("child"), input.join("elsewhere")).unwrap();
    request.selection.folder = Some("child".into());
    assert!(session.ensure(&request, &Default::default()).is_err());
    assert_eq!(
        session.claims().unwrap()[0].claim.entry_uuid,
        first.claim.entry_uuid
    );
    let after = serde_json::to_value(
        store
            .records()
            .unwrap()
            .into_iter()
            .filter(|record| matches!(record, Record::Acquisition(_)))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(before, after);
}

#[test]
fn revoked_access_during_fresh_fetch_cannot_publish_caller_destination() {
    use saucepan::core::{Store, claim::session::ApplicationConfiguration};
    use std::sync::{Arc, Mutex};
    let edit: Arc<Mutex<Option<(Store, uuid::Uuid)>>> = Arc::new(Mutex::new(None));
    let writer = edit.clone();
    let server = support::Server::new(move |_, _| {
        let (store, app) = writer.lock().unwrap().as_ref().unwrap().clone();
        store
            .configure_application(
                app,
                ApplicationConfiguration {
                    expected_revision: store.application(app).unwrap().revision,
                    credential: None,
                    policy_refs: policy_fixture::register(
                        &store,
                        AccessPolicy {
                            actions: Some(Default::default()),
                            ..Default::default()
                        },
                        Default::default(),
                    )
                    .unwrap(),
                },
                None,
            )
            .unwrap();
        (200, b"payload".to_vec())
    });
    let temp = tempfile::tempdir().unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Url {
            url: format!("{}/file", server.base),
        })
        .unwrap();
    let app = store.register_app("app".into()).unwrap().uuid;
    *edit.lock().unwrap() = Some((store.clone(), app));
    let destination = temp.path().join("download");
    let request = RetrieveRequest {
        source: SourceInput::Registered {
            source_uuid: source.uuid,
        },
        selector: Selector::Url,
        asset: Default::default(),
        destination: Some(destination.clone()),
        policy_refs: policy_fixture::definitions(
            &store,
            vec![serde_json::json!({"type":"cache_policy","mode":"no_store"})],
        )
        .unwrap(),
        expected_sha256: None,
    };
    let reader = RegisteredRetriever::open(store.root())
        .unwrap()
        .session(app, None)
        .unwrap();
    assert!(
        reader
            .retrieve(&request, None, &Default::default())
            .is_err()
    );
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn concurrent_automatic_ensures_share_claim_and_check_state() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, b"one").unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let app = store.register_app("app".into()).unwrap().uuid;
    let request = EnsureRequest {
        scope: scope("a"),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: Default::default(),
        policy_refs: policy_fixture::requested(
            &store,
            policy_fixture::Overrides {
                update_policy: Some(UpdatePolicy::MaxAge { seconds: 60 }),
                ..Default::default()
            },
        )
        .unwrap(),
    };
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let session = store.session(app, None).unwrap();
            let request = request.clone();
            std::thread::spawn(move || {
                session
                    .ensure(
                        &request,
                        &saucepan::core::source::AcquireOptions {
                            test_now: Some(100),
                            ..Default::default()
                        },
                    )
                    .unwrap()
            })
        })
        .collect();
    let claims: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert!(
        claims
            .iter()
            .all(|claim| claim.claim.uuid == claims[0].claim.uuid)
    );
    let states: Vec<_> = store
        .records()
        .unwrap()
        .into_iter()
        .filter_map(|record| match record {
            Record::Acquisition(state) => Some(state),
            _ => None,
        })
        .collect();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].revision, 1);
}

#[test]
fn rejected_compositions_do_not_register_sources_or_write_inside_the_store() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, b"payload").unwrap();
    let store = native_root::create(&temp.path().join("root")).unwrap();
    let mut request = RetrieveRequest {
        source: SourceInput::Inline {
            source: Source::Local { path: file.clone() },
        },
        selector: Selector::Local,
        asset: Default::default(),
        policy_refs: Default::default(),
        destination: None,
        expected_sha256: None,
    };
    let before = snapshot(store.root());
    let acquisition = AcquisitionRequest {
        destination: Destination::Directory {
            path: store.root().join("invalid-output"),
        },
        ..Default::default()
    };
    assert!(store.acquire_composed(&request, &acquisition).is_err());
    assert_eq!(snapshot(store.root()), before);
    policy_fixture::refresh(
        &store,
        &mut request,
        saucepan::models::policy::Freshness::CacheOnly {},
        false,
    );
    for update_policy in [
        UpdatePolicy::Always {},
        UpdatePolicy::MaxAge { seconds: 60 },
    ] {
        let policy_refs = policy_fixture::requested(
            &store,
            policy_fixture::Overrides {
                update_policy: Some(update_policy),
                ..Default::default()
            },
        )
        .unwrap();
        let before = snapshot(store.root());
        assert!(
            store
                .acquire_composed(
                    &request,
                    &AcquisitionRequest {
                        policy_refs: policy_refs.clone(),
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert_eq!(snapshot(store.root()), before);
    }
    let source = store.register_source(Source::Local { path: file }).unwrap();
    request.source = SourceInput::Registered {
        source_uuid: source.uuid,
    };
    request.destination = Some(temp.path().join("caller"));
    request.policy_refs = policy_fixture::definitions(
        &store,
        vec![serde_json::json!({"type":"cache_policy","mode":"no_store"})],
    )
    .unwrap();
    let reader = RegisteredRetriever::open(store.root()).unwrap();
    let material = reader.retrieve(&request, &at(100)).unwrap();
    let mut descriptor = material.descriptor().clone();
    let disguised = store.root().join("disguised");
    fs::write(&disguised, b"payload").unwrap();
    request.destination = Some(disguised.clone());
    request.policy_refs = policy_fixture::definitions(
        &store,
        vec![serde_json::json!({"type":"cache_policy","mode":"no_store"})],
    )
    .unwrap();
    descriptor.path = disguised;
    let before = snapshot(store.root());
    assert!(reader.open_caller(&request, &descriptor).is_err());
    assert_eq!(snapshot(store.root()), before);
}
