use saucepan::{
    core::{
        Store,
        claim::{
            registry::CredentialChange,
            security::{Authenticator, KeyProvider, MemoryKeys},
            session::{ApplicationConfiguration, ApplicationRegistration},
        },
    },
    error::{Error, ErrorCode},
    models::{
        claim::{
            Scope,
            policy::Action,
            security::{Credential, IntegrityMode},
        },
        source::Source,
        store::{Request, Selector, VerificationMode},
    },
};
use std::fs;
use uuid::Uuid;

fn scope(value: &str) -> Scope {
    value.to_string().try_into().unwrap()
}
fn input(key: &str) -> ApplicationRegistration {
    ApplicationRegistration {
        registration_key: scope(key),
        name: "same display name".into(),
        access: Default::default(),
        policies: Default::default(),
        credentials_required: false,
    }
}
fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}
fn setup(mode: IntegrityMode) -> (tempfile::TempDir, MemoryKeys, Store, Request) {
    let tmp = tempfile::tempdir().unwrap();
    let keys = MemoryKeys::default();
    let store =
        Store::create_with_integrity_and_keys(&tmp.path().join("root"), mode, &keys).unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, "content").unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    (tmp, keys, store, request)
}

#[test]
fn credentials_off_still_enforces_source_scope_action_and_owned_output_boundaries() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        credentials_off_still_enforces_source_scope_action_and_owned_output_boundaries_in(mode);
    }
}
fn credentials_off_still_enforces_source_scope_action_and_owned_output_boundaries_in(
    mode: IntegrityMode,
) {
    let (_tmp, _keys, store, request) = setup(mode);
    let mut registration = input("consumer");
    registration.access.sources = Some([request.source_uuid].into());
    registration.access.scopes = Some([scope("project-a")].into());
    let registered = store.register_application(&registration, None).unwrap();
    assert!(registered.credential.is_none());
    let session = store.session(registered.application.uuid, None).unwrap();
    let claim = session
        .acquire(
            &scope("project-a"),
            &request,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let held = session
        .lease_claim(claim.claim.uuid, VerificationMode::Metadata)
        .unwrap();
    session
        .lease_output(claim.claim.entry_uuid, VerificationMode::Metadata)
        .unwrap();
    assert_eq!(
        code(
            session
                .acquire(
                    &scope("project-b"),
                    &request,
                    &Default::default(),
                    &Default::default()
                )
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    let mut forbidden = request.clone();
    forbidden.source_uuid = Uuid::new_v4();
    assert_eq!(
        code(
            session
                .acquire(
                    &scope("project-a"),
                    &forbidden,
                    &Default::default(),
                    &Default::default()
                )
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    let other = store
        .register_application(&input("other-project"), None)
        .unwrap();
    let outsider = store.session(other.application.uuid, None).unwrap();
    assert_eq!(
        code(
            outsider
                .lease_output(claim.claim.entry_uuid, VerificationMode::Metadata)
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    assert!(held.directory().join("file").exists());
    let mut denied = registration.access.clone();
    denied.actions = Some([Action::Read, Action::Discover].into());
    store
        .configure_application(
            registered.application.uuid,
            ApplicationConfiguration {
                expected_revision: store
                    .application(registered.application.uuid)
                    .unwrap()
                    .revision,
                access: denied,
                policies: Default::default(),
                credential: None,
            },
            None,
        )
        .unwrap();
    assert_eq!(session.claims().unwrap().len(), 1);
    assert_eq!(
        code(
            session
                .update(claim.claim.uuid, &Default::default())
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    assert_eq!(
        code(session.release(claim.claim.uuid).unwrap_err()),
        ErrorCode::AccessDenied
    );
    assert_eq!(
        code(
            session
                .lease_output(claim.claim.entry_uuid, VerificationMode::Metadata)
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
}

#[test]
fn optional_proof_rotation_and_disable_are_atomic_and_do_not_revoke_existing_leases() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        optional_proof_rotation_and_disable_are_atomic_and_do_not_revoke_existing_leases_in(mode);
    }
}
fn optional_proof_rotation_and_disable_are_atomic_and_do_not_revoke_existing_leases_in(
    mode: IntegrityMode,
) {
    let (_tmp, keys, store, request) = setup(mode);
    let mut registration = input("protected");
    registration.credentials_required = true;
    let registered = store.register_application(&registration, None).unwrap();
    let app = registered.application.uuid;
    let token = registered.credential.unwrap();
    let proof = Credential::from_bytes(token.expose()).unwrap();
    let session = store.session(app, Some(token)).unwrap();
    assert_eq!(
        code(store.session(app, None).err().unwrap()),
        ErrorCode::AuthenticationRequired
    );
    assert_eq!(
        code(
            store
                .session(app, Some(Credential::generate().unwrap()))
                .err()
                .unwrap()
        ),
        ErrorCode::AuthenticationFailed
    );
    assert!(
        store
            .register_application(&registration, Some(&proof))
            .unwrap()
            .credential
            .is_none()
    );
    assert_eq!(
        code(
            store
                .register_application(&registration, None)
                .err()
                .unwrap()
        ),
        ErrorCode::AuthenticationRequired
    );
    let claimed = session
        .acquire(
            &scope("project"),
            &request,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let held = session
        .lease_claim(claimed.claim.uuid, VerificationMode::Metadata)
        .unwrap();
    assert_eq!(
        code(store.claims(app).unwrap_err()),
        ErrorCode::AuthenticationRequired
    );
    let rotated = store
        .configure_application(
            app,
            ApplicationConfiguration {
                expected_revision: store.application(app).unwrap().revision,
                access: Default::default(),
                policies: Default::default(),
                credential: Some(CredentialChange::Rotate),
            },
            Some(&proof),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        code(session.claims().unwrap_err()),
        ErrorCode::AuthenticationFailed
    );
    assert_eq!(
        code(
            session
                .update(Uuid::new_v4(), &Default::default())
                .unwrap_err()
        ),
        ErrorCode::AuthenticationFailed
    );
    assert_eq!(
        code(session.release(Uuid::new_v4()).unwrap_err()),
        ErrorCode::AuthenticationFailed
    );
    assert_eq!(
        code(
            session
                .lease_claim(Uuid::new_v4(), VerificationMode::Metadata)
                .err()
                .unwrap()
        ),
        ErrorCode::AuthenticationFailed
    );
    let new_proof = Credential::from_bytes(rotated.expose()).unwrap();
    let current = store.session(app, Some(rotated)).unwrap();
    assert_eq!(current.claims().unwrap().len(), 1);
    assert_eq!(fs::read(held.directory().join("file")).unwrap(), b"content");
    store
        .configure_application(
            app,
            ApplicationConfiguration {
                expected_revision: store.application(app).unwrap().revision,
                access: Default::default(),
                policies: Default::default(),
                credential: Some(CredentialChange::Disable),
            },
            Some(&new_proof),
        )
        .unwrap();
    store.session(app, None).unwrap().claims().unwrap();
    let master = keys
        .load(
            Authenticator::store_hint(&fs::read(store.root().join("root.json")).unwrap()).unwrap(),
        )
        .unwrap();
    for path in metadata_files(store.root()) {
        let bytes = fs::read(&path).unwrap();
        for secret in [
            Some(proof.expose()),
            Some(new_proof.expose()),
            master.as_ref().map(|key| key.expose()),
        ]
        .into_iter()
        .flatten()
        {
            assert!(!bytes.windows(32).any(|window| window == secret));
            let encoded = serde_json::to_vec(secret).unwrap();
            assert!(!bytes.windows(encoded.len()).any(|window| window == encoded));
        }
    }
    Store::open_with_keys(store.root(), &keys)
        .unwrap()
        .claims(app)
        .unwrap();
}

#[test]
fn readable_registry_is_authoritative_and_missing_or_edited_members_stop_access_and_reclaim() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        readable_registry_is_authoritative_and_missing_or_edited_members_stop_access_and_reclaim_in(
            mode,
        );
    }
}
fn readable_registry_is_authoritative_and_missing_or_edited_members_stop_access_and_reclaim_in(
    mode: IntegrityMode,
) {
    let (_tmp, keys, store, request) = setup(mode);
    let registered = store
        .register_application(&input("registry"), None)
        .unwrap();
    let app = registered.application.uuid;
    let session = store.session(app, None).unwrap();
    let context = [("project".into(), "human-readable".into())].into();
    let claimed = session
        .acquire(&scope("project"), &request, &context, &Default::default())
        .unwrap();
    let app_bytes =
        fs::read_to_string(store.root().join(format!("index/{app}/index.json"))).unwrap();
    assert!(!app_bytes.contains("human-readable"));
    let member = store
        .root()
        .join(format!("claims/{app}/{}.json", claimed.claim.uuid));
    let original = fs::read(&member).unwrap();
    assert!(String::from_utf8_lossy(&original).contains("human-readable"));
    fs::write(
        &member,
        String::from_utf8_lossy(&original).replace("human-readable", "edited"),
    )
    .unwrap();
    assert_eq!(
        code(session.claims().unwrap_err()),
        ErrorCode::IntegrityError
    );
    assert_eq!(
        code(store.reclaim().unwrap_err()),
        ErrorCode::IntegrityError
    );
    fs::remove_file(&member).unwrap();
    assert_eq!(
        code(Store::open_with_keys(store.root(), &keys).err().unwrap()),
        ErrorCode::IntegrityError
    );
    fs::write(&member, original).unwrap();
    session.claims().unwrap();
    let registry = store.root().join(format!("claims/{app}/index.json"));
    fs::remove_file(registry).unwrap();
    assert_eq!(
        code(store.reclaim().unwrap_err()),
        ErrorCode::IntegrityError
    );
}

#[test]
fn registration_keys_are_idempotent_and_policy_changes_cannot_invalidate_surviving_claims() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        registration_keys_are_idempotent_and_policy_changes_cannot_invalidate_surviving_claims_in(
            mode,
        );
    }
}
fn registration_keys_are_idempotent_and_policy_changes_cannot_invalidate_surviving_claims_in(
    mode: IntegrityMode,
) {
    let (_tmp, _keys, store, request) = setup(mode);
    let registration = input("explicit-key");
    let first = store.register_application(&registration, None).unwrap();
    let same = store.register_application(&registration, None).unwrap();
    assert_eq!(first.application.uuid, same.application.uuid);
    let different = store
        .register_application(&input("different-key"), None)
        .unwrap();
    assert_ne!(first.application.uuid, different.application.uuid);
    let session = store.session(first.application.uuid, None).unwrap();
    session
        .acquire(
            &scope("project"),
            &request,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let before = fs::read(store.root().join("root.json")).unwrap();
    let mut policies = registration.policies.clone();
    policies.maximum_keep = 1;
    policies.defaults.output.eviction = saucepan::models::policy::Eviction::Latest { keep: 1 };
    assert_eq!(
        code(
            store
                .configure_application(
                    first.application.uuid,
                    ApplicationConfiguration {
                        expected_revision: store
                            .application(first.application.uuid)
                            .unwrap()
                            .revision,
                        access: Default::default(),
                        policies,
                        credential: None
                    },
                    None
                )
                .err()
                .unwrap()
        ),
        ErrorCode::PolicyConflict
    );
    assert_eq!(fs::read(store.root().join("root.json")).unwrap(), before);
}

#[test]
fn generic_discovery_only_exposes_public_permitted_scopes_and_grants_no_output_access() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        generic_discovery_only_exposes_public_permitted_scopes_and_grants_no_output_access_in(mode);
    }
}
fn generic_discovery_only_exposes_public_permitted_scopes_and_grants_no_output_access_in(
    mode: IntegrityMode,
) {
    let (_tmp, _keys, store, request) = setup(mode);
    let mut registration = input("discoverable");
    registration.access.public_scopes.insert(scope("public"));
    let registered = store.register_application(&registration, None).unwrap();
    let session = store.session(registered.application.uuid, None).unwrap();
    let public = session
        .acquire(
            &scope("public"),
            &request,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    session
        .acquire(
            &scope("private"),
            &request,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let discovery = saucepan::core::query::DiscoveryQuery {
        scope: None,
        conditions: vec![],
    };
    let found = store.discover_claims(&discovery).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].claims.len(), 1);
    assert_eq!(found[0].claims[0].claim.uuid, public.claim.uuid);
    assert_eq!(
        session
            .query(&saucepan::core::query::ClaimQuery {
                app: session.app(),
                scope: Some(scope("private")),
                conditions: vec![]
            })
            .unwrap()
            .claims
            .len(),
        1
    );
    let outsider = store
        .register_application(&input("outsider"), None)
        .unwrap();
    assert_eq!(
        code(
            store
                .session(outsider.application.uuid, None)
                .unwrap()
                .lease_output(public.claim.entry_uuid, VerificationMode::Metadata)
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    store
        .configure_application(
            session.app(),
            ApplicationConfiguration {
                expected_revision: store.application(session.app()).unwrap().revision,
                access: registration.access,
                policies: Default::default(),
                credential: Some(CredentialChange::Enable),
            },
            None,
        )
        .unwrap();
    assert!(store.discover_claims(&discovery).unwrap().is_empty());
}

fn metadata_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(metadata_files(&path));
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    files
}

#[test]
fn current_protected_proof_survives_open_and_rotates_to_independent_proof() {
    use saucepan::{
        core::claim::{
            persistence::Metadata,
            security::{Identity, Kind},
        },
        models::claim::security::CredentialPolicy,
        utils::crypto,
    };
    let (_tmp, keys, store, _) = setup(IntegrityMode::Keyring);
    let registered = store
        .register_application(&input("existing-proof"), None)
        .unwrap();
    let app = registered.application.uuid;
    let token = Credential::generate().unwrap();
    let store_id =
        Authenticator::store_hint(&fs::read(store.root().join("root.json")).unwrap()).unwrap();
    let master = keys.load(store_id).unwrap().unwrap();
    let derived = crypto::derive_key::<32>(
        master.expose(),
        Some(store_id.as_bytes()),
        b"saucepan/credentials/hmac-sha256",
    )
    .unwrap();
    let mut framed = Vec::new();
    framed.extend_from_slice(app.as_bytes());
    framed.extend_from_slice(&1u64.to_be_bytes());
    framed.extend_from_slice(token.expose());
    let verifier = crypto::authenticate(derived.as_ref(), framed.as_slice()).unwrap();
    let metadata = Metadata::open(store.root(), &keys).unwrap();
    let snapshot = metadata.snapshot().unwrap();
    let identity = Identity {
        kind: Kind::Registry,
        owner: Some(app),
        member: None,
    };
    let mut registry: saucepan::models::claim::Registry =
        snapshot.read(metadata.authenticator(), identity).unwrap();
    registry.credential = CredentialPolicy::Required {
        generation: 1,
        verifier,
    };
    metadata
        .publish(
            &snapshot,
            vec![(identity, serde_json::to_value(&registry).unwrap())],
            vec![],
        )
        .unwrap();
    let before = fs::read(store.root().join("root.json")).unwrap();
    let reopened = Store::open_with_keys(store.root(), &keys).unwrap();
    reopened
        .session(app, Some(Credential::from_bytes(token.expose()).unwrap()))
        .unwrap();
    assert_eq!(before, fs::read(store.root().join("root.json")).unwrap());
    let replacement = reopened
        .configure_application(
            app,
            ApplicationConfiguration {
                expected_revision: reopened.application(app).unwrap().revision,
                access: Default::default(),
                policies: Default::default(),
                credential: Some(CredentialChange::Rotate),
            },
            Some(&token),
        )
        .unwrap()
        .unwrap();
    assert!(reopened.session(app, Some(token)).is_err());
    reopened.session(app, Some(replacement)).unwrap();
    let snapshot = metadata.snapshot().unwrap();
    let changed: saucepan::models::claim::Registry =
        snapshot.read(metadata.authenticator(), identity).unwrap();
    assert!(matches!(
        changed.credential,
        CredentialPolicy::Bearer { generation: 2, .. }
    ));
    assert!(
        Authenticator::basic(store_id)
            .check_credential(app, &registry.credential, None)
            .is_err()
    );
}
