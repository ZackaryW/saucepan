use super::*;
use crate::{
    core::{
        claim::session::{ApplicationConfiguration, ApplicationRegistration},
        query::DiscoveryQuery,
    },
    models::{
        claim::{
            EnsureRequest,
            policy::{AccessPolicy, Action},
            security::Credential,
        },
        source::{Discovery, Resolved},
        store::{Selector, VerificationMode},
    },
};
use serde_json::json;
use std::{collections::BTreeSet, fs};

fn registration(
    store: &Store,
    key: &str,
    access: AccessPolicy,
    credentials_required: bool,
) -> ApplicationRegistration {
    ApplicationRegistration {
        registration_key: key.to_owned().try_into().unwrap(),
        name: key.into(),
        credentials_required,
        policy_refs: crate::policy_fixture::register(store, access, Default::default()).unwrap(),
    }
}
fn query(value: serde_json::Value) -> SourceQuery {
    serde_json::from_value(value).unwrap()
}
fn code<T>(result: Result<T>, expected: ErrorCode) {
    let Err(error) = result else {
        panic!("expected failure")
    };
    assert_eq!(Error::from_anyhow(&error).code, expected, "{error:?}");
}
fn release(store: &Store) -> SourceRecord {
    store
        .register_source(Source::GithubRelease {
            repo: "owner/project".into(),
        })
        .unwrap()
}

#[test]
fn source_visibility_is_private_explicit_preserved_and_independent_of_claims() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let mut source = release(&store);
    assert!(!source.public_discovery);
    source.discovery.insert(
        "latest".into(),
        Discovery {
            checked_at: 1,
            resolved: Resolved::Local,
        },
    );
    store.save_source(&source).unwrap();
    let all = SourceQuery::default();
    assert!(store.query_sources(&all).unwrap().sources.is_empty());
    let published = store.set_source_visibility(source.uuid, true).unwrap();
    assert_eq!(published.discovery["latest"].checked_at, 1);
    assert_eq!(published.source, source.source);
    assert!(
        store
            .register_source(source.source)
            .unwrap()
            .public_discovery
    );
    let result = store.query_sources(&all).unwrap();
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.sources[0].uuid, source.uuid);
    assert!(
        store
            .discover_claims(&DiscoveryQuery {
                scope: None,
                conditions: vec![]
            })
            .unwrap()
            .is_empty()
    );
    let app = store.register_app("app".into()).unwrap();
    code(
        store
            .session(app.uuid, None)
            .unwrap()
            .into_store()
            .set_source_visibility(source.uuid, false),
        ErrorCode::AccessDenied,
    );
    store.set_source_visibility(source.uuid, false).unwrap();
    assert!(store.query_sources(&all).unwrap().sources.is_empty());
    // Omitted visibility in an authenticated record is private, not an unsigned fallback.
    let identity = crate::core::index::source_identity(source.uuid);
    let snapshot = store.metadata.snapshot().unwrap();
    let mut value = serde_json::to_value(store.source(source.uuid).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("public_discovery");
    store
        .metadata
        .publish(&snapshot, vec![(identity, value)], vec![])
        .unwrap();
    assert!(!store.source(source.uuid).unwrap().public_discovery);
}

#[test]
fn source_filters_find_unclaimed_metadata_without_payload_or_refresh() {
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let release = release(&store);
    let git = store
        .register_source(Source::GithubRepo {
            repo: "Owner/Repo".into(),
        })
        .unwrap();
    let url = store
        .register_source(Source::Url {
            url: "http://127.0.0.1:1/unreachable".into(),
        })
        .unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "data").unwrap();
    let local = store
        .register_source(Source::Local { path: file.clone() })
        .unwrap();
    fs::remove_file(file).unwrap();
    store.access_observer = Some(std::sync::Arc::new(|_| panic!("query touched payload")));
    let app = store.register_app("simple".into()).unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let before = store.metadata.snapshot().unwrap().digest;
    let found = session.query_sources(&SourceQuery::default()).unwrap();
    let mut ids = vec![release.uuid, git.uuid, url.uuid, local.uuid];
    ids.sort();
    assert_eq!(
        found.sources.iter().map(|s| s.uuid).collect::<Vec<_>>(),
        ids
    );
    for (filter, id) in [
        (
            json!({"type":"github_release","repo":"owner/project"}),
            release.uuid,
        ),
        (
            json!({"type":"git","origin":"https://github.com/owner/repo.git"}),
            git.uuid,
        ),
        (
            json!({"type":"url","url":"http://127.0.0.1:1/unreachable"}),
            url.uuid,
        ),
        (json!({"type":"local","uuid":local.uuid}), local.uuid),
    ] {
        let result = session.query_sources(&query(filter)).unwrap();
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.sources[0].uuid, id);
    }
    for filter in [
        json!({"repo":"Owner/project"}),
        json!({"type":"local","repo":"owner/project"}),
        json!({"repo":"*"}),
    ] {
        assert!(
            session
                .query_sources(&query(filter))
                .unwrap()
                .sources
                .is_empty()
        );
    }
    assert_eq!(before, store.metadata.snapshot().unwrap().digest);
    assert_eq!(
        found.generation,
        store.metadata.snapshot().unwrap().catalog.generation
    );
    assert!(session.claims().unwrap().is_empty());
    assert!(store.source(release.uuid).unwrap().discovery.is_empty());
    assert!(store.source(git.uuid).unwrap().discovery.is_empty());
}

#[test]
fn source_filters_are_strict_even_with_empty_catalogue() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    for value in [
        json!({"scope":"default"}),
        json!({"where":[]}),
        json!({"uuid":"bad"}),
        json!({"type":"github_repo"}),
        json!({"type":"unknown"}),
        json!({"repo":false}),
    ] {
        assert!(serde_json::from_value::<SourceQuery>(value).is_err());
    }
    for value in [
        json!({"repo":""}),
        json!({"origin":"é".repeat(2049)}),
        json!({"url":""}),
    ] {
        code(
            store.query_sources(&query(value)),
            ErrorCode::InvalidRequest,
        );
    }
    assert!(
        store
            .query_sources(&query(json!({"repo":"x".repeat(4096)})))
            .unwrap()
            .sources
            .is_empty()
    );
}

#[test]
fn source_queries_enforce_app_allowances_without_weakening_claim_scopes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "data").unwrap();
    let local = store.register_source(Source::Local { path: file }).unwrap();
    let other = release(&store);
    store.set_source_visibility(other.uuid, true).unwrap();
    let registered = store
        .register_application(
            &registration(
                &store,
                "restricted",
                AccessPolicy {
                    sources: Some(BTreeSet::from([local.uuid])),
                    scopes: Some(BTreeSet::new()),
                    ..Default::default()
                },
                false,
            ),
            None,
        )
        .unwrap();
    let app = registered.application.uuid;
    let session = store.session(app, None).unwrap();
    let found = session.query_sources(&SourceQuery::default()).unwrap();
    assert_eq!(found.sources.len(), 1);
    assert_eq!(found.sources[0].uuid, local.uuid);
    let request = EnsureRequest {
        scope: "default".to_string().try_into().unwrap(),
        source_uuid: local.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: Default::default(),
        policy_refs: Default::default(),
    };
    code(
        session.ensure(&request, &Default::default()),
        ErrorCode::AccessDenied,
    );
    let mut settings = store.application(app).unwrap().settings;
    settings.source_filter = vec![other.uuid];
    store.set_settings(app, settings).unwrap();
    assert!(
        session
            .query_sources(&SourceQuery::default())
            .unwrap()
            .sources
            .is_empty()
    );
    let empty = store
        .register_application(
            &registration(
                &store,
                "empty",
                AccessPolicy {
                    sources: Some(BTreeSet::new()),
                    ..Default::default()
                },
                false,
            ),
            None,
        )
        .unwrap();
    assert!(
        store
            .session(empty.application.uuid, None)
            .unwrap()
            .query_sources(&SourceQuery::default())
            .unwrap()
            .sources
            .is_empty()
    );
    let denied = store
        .register_application(
            &registration(
                &store,
                "denied",
                AccessPolicy {
                    actions: Some(BTreeSet::from([Action::Acquire])),
                    ..Default::default()
                },
                false,
            ),
            None,
        )
        .unwrap();
    code(
        store
            .session(denied.application.uuid, None)
            .unwrap()
            .query_sources(&SourceQuery::default()),
        ErrorCode::AccessDenied,
    );
}

#[test]
fn source_queries_check_proof_and_current_policy_even_without_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let reg = store
        .register_application(
            &registration(&store, "proof", Default::default(), true),
            None,
        )
        .unwrap();
    let app = reg.application.uuid;
    code(store.session(app, None), ErrorCode::AuthenticationRequired);
    code(
        store.session(app, Some(Credential::generate().unwrap())),
        ErrorCode::AuthenticationFailed,
    );
    let proof = reg.credential.unwrap();
    let session = store
        .session(app, Some(Credential::from_bytes(proof.expose()).unwrap()))
        .unwrap();
    assert!(
        session
            .query_sources(&SourceQuery::default())
            .unwrap()
            .sources
            .is_empty()
    );
    store
        .configure_application(
            app,
            ApplicationConfiguration {
                expected_revision: store.application(app).unwrap().revision,
                credential: None,
                policy_refs: crate::policy_fixture::register(
                    &store,
                    AccessPolicy {
                        actions: Some(BTreeSet::new()),
                        ..Default::default()
                    },
                    Default::default(),
                )
                .unwrap(),
            },
            Some(&proof),
        )
        .unwrap();
    code(
        session.query_sources(&SourceQuery::default()),
        ErrorCode::AccessDenied,
    );
    // A credential enabled after a credential-free session was opened is rechecked.
    let plain = store.register_app("plain".into()).unwrap();
    let old_session = store.session(plain.uuid, None).unwrap();
    store
        .configure_application(
            plain.uuid,
            ApplicationConfiguration {
                expected_revision: plain.revision,
                credential: Some(crate::core::claim::registry::CredentialChange::Enable),
                policy_refs: crate::policy_fixture::register(
                    &store,
                    Default::default(),
                    Default::default(),
                )
                .unwrap(),
            },
            None,
        )
        .unwrap();
    code(
        old_session.query_sources(&SourceQuery::default()),
        ErrorCode::AuthenticationRequired,
    );
}

#[test]
fn source_queries_fail_on_hidden_tampered_or_missing_metadata() {
    for missing in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let source = release(&store);
        let path = store.root().join(format!("source/{}.json", source.uuid));
        if missing {
            fs::remove_file(path).unwrap();
        } else {
            fs::write(path, b"{}").unwrap();
        }
        code(
            store.query_sources(&SourceQuery::default()),
            ErrorCode::IntegrityError,
        );
    }
}

#[test]
fn visibility_update_waits_for_in_flight_provider_publication() {
    use crate::utils::lock::{LockGuard, LockMode};
    use std::{sync::mpsc, time::Duration};
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let mut source = release(&store);
    let provider_lock = LockGuard::acquire(
        &store.root().join(format!(".locks/source-{}", source.uuid)),
        LockMode::Exclusive,
        Duration::from_secs(1),
    )
    .unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let publisher = store.clone();
    let id = source.uuid;
    let handle = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = publisher.set_source_visibility(id, true);
        done_tx.send(()).unwrap();
        result.unwrap()
    });
    started_rx.recv().unwrap();
    assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
    source.discovery.insert(
        "latest".into(),
        Discovery {
            checked_at: 42,
            resolved: Resolved::Local,
        },
    );
    {
        let _guard = store.lock().unwrap();
        store.save_source(&source).unwrap();
    }
    drop(provider_lock);
    let published = handle.join().unwrap();
    assert!(published.public_discovery);
    assert_eq!(published.discovery["latest"].checked_at, 42);
    assert!(store.source(id).unwrap().public_discovery);
}

#[test]
fn public_source_and_public_claim_visibility_do_not_grant_ownership() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "data").unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let scope = "default".to_string().try_into().unwrap();
    let registered = store
        .register_application(
            &registration(
                &store,
                "owner",
                AccessPolicy {
                    public_scopes: BTreeSet::from([scope]),
                    ..Default::default()
                },
                false,
            ),
            None,
        )
        .unwrap();
    let owner = store.session(registered.application.uuid, None).unwrap();
    let request: EnsureRequest = serde_json::from_value(
        json!({"scope":"default","source_uuid":source.uuid,"selector":{"type":"local"}}),
    )
    .unwrap();
    let claimed = owner.ensure(&request, &Default::default()).unwrap();
    assert_eq!(
        store
            .discover_claims(&DiscoveryQuery {
                scope: None,
                conditions: vec![]
            })
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .query_sources(&SourceQuery::default())
            .unwrap()
            .sources
            .is_empty()
    );
    store.set_source_visibility(source.uuid, true).unwrap();
    let outsider = store.register_app("outsider".into()).unwrap();
    let outsider = store.session(outsider.uuid, None).unwrap();
    assert_eq!(
        outsider
            .query_sources(&SourceQuery::default())
            .unwrap()
            .sources
            .len(),
        1
    );
    code(
        outsider.lease_output(claimed.claim.entry_uuid, VerificationMode::Metadata),
        ErrorCode::AccessDenied,
    );
    assert!(outsider.claims().unwrap().is_empty());
}
