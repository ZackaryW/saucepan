use saucepan::{
    core::claim::{
        persistence::{Metadata, Publication, record_path},
        security::{Authenticator, Identity, KeyProvider, Kind, MemoryKeys, NativeKeyring, Secret},
    },
    error::{Error, ErrorCode, Result},
    models::claim::security::{Credential, CredentialPolicy, IntegrityMode},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

fn id(kind: Kind, owner: Option<Uuid>, member: Option<Uuid>) -> Identity {
    Identity {
        kind,
        owner,
        member,
    }
}
fn root(path: &Path) {
    for part in ["source", "index", "claims", "store", "bin", ".staging"] {
        fs::create_dir_all(path.join(part)).unwrap();
    }
}

#[test]
fn readable_records_bind_exact_bytes_to_store_app_member_kind_and_generation() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        readable_records_bind_exact_bytes_to_store_app_member_kind_and_generation_in(mode);
    }
}
fn readable_records_bind_exact_bytes_to_store_app_member_kind_and_generation_in(
    mode: IntegrityMode,
) {
    let keys = MemoryKeys::default();
    let auth = test_auth(mode, Uuid::new_v4(), &keys);
    let identity = id(Kind::Claim, Some(Uuid::new_v4()), Some(Uuid::new_v4()));
    let record = auth
        .sign(identity, 8, &json!({"context":{"project":"visible"}}))
        .unwrap();
    assert!(String::from_utf8_lossy(&record).contains("visible"));
    assert_eq!(auth.verify::<Value>(identity, &record).unwrap().0, 8);
    for replacement in [
        id(Kind::Claim, Some(Uuid::new_v4()), identity.member),
        id(Kind::Claim, identity.owner, Some(Uuid::new_v4())),
        id(Kind::Registry, identity.owner, identity.member),
    ] {
        assert_eq!(
            auth.verify::<Value>(replacement, &record).unwrap_err().code,
            ErrorCode::IntegrityError
        );
    }
    let other = test_auth(mode, Uuid::new_v4(), &keys);
    assert_eq!(
        other.verify::<Value>(identity, &record).unwrap_err().code,
        ErrorCode::IntegrityError
    );
    // Same decoded payload, different literal bytes: reformatting is an edit.
    let compact = serde_json::to_vec(&serde_json::from_slice::<Value>(&record).unwrap()).unwrap();
    assert!(auth.verify::<Value>(identity, &compact).is_err());
    for (field, value) in [
        ("generation", json!(9)),
        ("store", json!(Uuid::new_v4())),
        ("payload", json!({"context":{}})),
        ("extra", json!(true)),
    ] {
        let mut edited: Value = serde_json::from_slice(&record).unwrap();
        edited[field] = value;
        assert!(
            auth.verify::<Value>(identity, &serde_json::to_vec(&edited).unwrap())
                .is_err()
        );
    }
    assert!(
        auth.verify::<Value>(identity, &record[..record.len() - 1])
            .is_err()
    );
    let duplicate =
        String::from_utf8(record.clone())
            .unwrap()
            .replacen("{", "{\"generation\":8,", 1);
    assert!(
        auth.verify::<Value>(identity, duplicate.as_bytes())
            .is_err()
    );
    assert!(
        auth.verify::<Value>(identity, br#"{"context":"unsigned"}"#)
            .is_err()
    );
}

#[test]
fn proof_is_optional_bound_to_app_and_store_and_rotation_invalidates_previous_proof() {
    let keys = MemoryKeys::default();
    let auth = Authenticator::provision(Uuid::new_v4(), &keys).unwrap();
    let app = Uuid::new_v4();
    let first = Credential::generate().unwrap();
    let second = Credential::generate().unwrap();
    auth.check_credential(app, &CredentialPolicy::Disabled, None)
        .unwrap();
    let policy = auth.credential_policy(app, 1, &first).unwrap();
    assert_eq!(
        auth.check_credential(app, &policy, None).unwrap_err().code,
        ErrorCode::AuthenticationRequired
    );
    auth.check_credential(app, &policy, Some(&first)).unwrap();
    assert_eq!(
        auth.check_credential(app, &policy, Some(&second))
            .unwrap_err()
            .code,
        ErrorCode::AuthenticationFailed
    );
    assert!(
        auth.check_credential(Uuid::new_v4(), &policy, Some(&first))
            .is_err()
    );
    let rotated = auth.credential_policy(app, 2, &second).unwrap();
    assert!(auth.check_credential(app, &rotated, Some(&first)).is_err());
    auth.check_credential(app, &rotated, Some(&second)).unwrap();
    let other = Authenticator::provision(Uuid::new_v4(), &keys).unwrap();
    assert!(other.check_credential(app, &policy, Some(&first)).is_err());
    let serialized = serde_json::to_vec(&rotated).unwrap();
    assert!(!serialized.windows(32).any(|w| w == second.expose()));
    assert!(!format!("{first:?} {auth:?}").contains(&format!("{:?}", first.expose())));
}

#[test]
fn catalog_detects_deletion_substitution_tampering_and_unknown_records() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        catalog_detects_deletion_substitution_tampering_and_unknown_records_in(mode);
    }
}
fn catalog_detects_deletion_substitution_tampering_and_unknown_records_in(mode: IntegrityMode) {
    let tmp = tempfile::tempdir().unwrap();
    root(tmp.path());
    let keys = MemoryKeys::default();
    let metadata = Metadata::initialize_with_mode(tmp.path(), mode, &keys).unwrap();
    let app = Uuid::new_v4();
    let a = id(Kind::Application, Some(app), None);
    let registry = id(Kind::Registry, Some(app), None);
    let member = id(Kind::Claim, Some(app), Some(Uuid::new_v4()));
    metadata
        .publish(
            &metadata.snapshot().unwrap(),
            vec![
                (a, json!({"name":"app"})),
                (registry, json!({"members":[member.member]})),
                (member, json!({"scope":"project"})),
            ],
            vec![],
        )
        .unwrap();
    let snapshot = metadata.snapshot().unwrap();
    assert_eq!(
        snapshot.read::<Value>(metadata.authenticator(), a).unwrap()["name"],
        "app"
    );
    let unknown = id(Kind::Claim, Some(app), Some(Uuid::new_v4()));
    assert_eq!(
        snapshot
            .read::<Value>(metadata.authenticator(), unknown)
            .unwrap_err()
            .code,
        ErrorCode::UnknownClaim
    );
    for identity in [a, registry, member] {
        let path = tmp.path().join(record_path(identity).unwrap());
        let original = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(
            metadata.snapshot().err().unwrap().code,
            ErrorCode::IntegrityError
        );
        fs::write(&path, br#"{"unsigned":true}"#).unwrap();
        assert_eq!(
            metadata.snapshot().err().unwrap().code,
            ErrorCode::IntegrityError
        );
        fs::write(&path, &original).unwrap();
        metadata.snapshot().unwrap();
    }
    // A legitimate older record cannot be substituted into a newer catalog.
    let path = tmp.path().join(record_path(a).unwrap());
    let old = fs::read(&path).unwrap();
    metadata
        .publish(&snapshot, vec![(a, json!({"name":"updated"}))], vec![])
        .unwrap();
    fs::write(path, old).unwrap();
    assert_eq!(
        metadata.snapshot().err().unwrap().code,
        ErrorCode::IntegrityError
    );
}

#[test]
fn every_publication_boundary_recovers_one_generation_and_replays_idempotently() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        every_publication_boundary_recovers_one_generation_and_replays_idempotently_in(mode);
    }
}
fn every_publication_boundary_recovers_one_generation_and_replays_idempotently_in(
    mode: IntegrityMode,
) {
    // Journal, three record replacements, a deletion, catalog, journal removal.
    for fail_at in 0..7 {
        let tmp = tempfile::tempdir().unwrap();
        root(tmp.path());
        let keys = MemoryKeys::default();
        let metadata = Metadata::initialize_with_mode(tmp.path(), mode, &keys).unwrap();
        let app = Uuid::new_v4();
        let a = id(Kind::Application, Some(app), None);
        let registry = id(Kind::Registry, Some(app), None);
        let old_member = id(Kind::Claim, Some(app), Some(Uuid::new_v4()));
        let new_member = id(Kind::Claim, Some(app), Some(Uuid::new_v4()));
        metadata
            .publish(
                &metadata.snapshot().unwrap(),
                vec![(a, json!(0)), (registry, json!(0)), (old_member, json!(0))],
                vec![],
            )
            .unwrap();
        let mut position = 0;
        let result = metadata.publish_observed(
            &metadata.snapshot().unwrap(),
            vec![(a, json!(1)), (registry, json!(1)), (new_member, json!(1))],
            vec![old_member],
            |_| {
                let current = position;
                position += 1;
                if current == fail_at {
                    Err(Error::new(ErrorCode::IoError, "injected interruption"))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "boundary {fail_at} not exercised");
        let reopened = Metadata::open(tmp.path(), &keys).unwrap();
        reopened.recover().unwrap();
        reopened.recover().unwrap();
        let snapshot = reopened.snapshot().unwrap();
        assert_eq!(snapshot.catalog.generation, 2);
        for identity in [a, registry, new_member] {
            assert_eq!(
                snapshot
                    .read::<Value>(reopened.authenticator(), identity)
                    .unwrap(),
                json!(1)
            );
        }
        assert!(!snapshot.contains(old_member).unwrap());
        assert!(!tmp.path().join(record_path(old_member).unwrap()).exists());
        assert!(
            !tmp.path()
                .join(".staging/metadata-transaction.json")
                .exists()
        );
    }
}

#[test]
fn journal_tampering_is_rejected_before_any_metadata_effect() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        journal_tampering_is_rejected_before_any_metadata_effect_in(mode);
    }
}
fn journal_tampering_is_rejected_before_any_metadata_effect_in(mode: IntegrityMode) {
    let tmp = tempfile::tempdir().unwrap();
    root(tmp.path());
    let keys = MemoryKeys::default();
    let metadata = Metadata::initialize_with_mode(tmp.path(), mode, &keys).unwrap();
    let identity = id(Kind::Application, Some(Uuid::new_v4()), None);
    assert!(
        metadata
            .publish_observed(
                &metadata.snapshot().unwrap(),
                vec![(identity, json!({"name":"old"}))],
                vec![],
                |point| {
                    assert_eq!(point, Publication::Journal);
                    Err(Error::new(ErrorCode::IoError, "interrupted"))
                }
            )
            .is_err()
    );
    let catalog_before = fs::read(tmp.path().join("root.json")).unwrap();
    let path = tmp.path().join(".staging/metadata-transaction.json");
    let bytes = fs::read_to_string(&path).unwrap().replace("old", "new");
    fs::write(path, bytes).unwrap();
    assert_eq!(
        metadata.recover().unwrap_err().code,
        ErrorCode::IntegrityError
    );
    assert_eq!(
        fs::read(tmp.path().join("root.json")).unwrap(),
        catalog_before
    );
    assert!(!tmp.path().join(record_path(identity).unwrap()).exists());
}

struct FailingKeys {
    keys: MemoryKeys,
    failing: AtomicBool,
}
impl KeyProvider for FailingKeys {
    fn load(&self, id: Uuid) -> Result<Option<Secret>> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(Error::new(
                ErrorCode::KeyUnavailable,
                "test service unavailable",
            ));
        }
        self.keys.load(id)
    }
    fn insert(&self, id: Uuid, key: &Secret) -> Result<()> {
        self.keys.insert(id, key)
    }
}

#[test]
fn initialization_resumes_identity_but_open_never_replaces_a_missing_key() {
    let tmp = tempfile::tempdir().unwrap();
    root(tmp.path());
    let keys = FailingKeys {
        keys: MemoryKeys::default(),
        failing: AtomicBool::new(true),
    };
    assert_eq!(
        Metadata::initialize(tmp.path(), &keys).err().unwrap().code,
        ErrorCode::KeyUnavailable
    );
    let reserved: Uuid = serde_json::from_value(
        serde_json::from_slice::<Value>(
            &fs::read(tmp.path().join(".staging/initializing.json")).unwrap(),
        )
        .unwrap()["store"]
            .clone(),
    )
    .unwrap();
    keys.failing.store(false, Ordering::SeqCst);
    // Simulate a crash after key provisioning but before root publication.
    Authenticator::provision(reserved, &keys).unwrap();
    let metadata = Metadata::initialize(tmp.path(), &keys).unwrap();
    assert_eq!(metadata.authenticator().store(), reserved);
    let bytes = fs::read(tmp.path().join("root.json")).unwrap();
    assert_eq!(
        Metadata::open(tmp.path(), &MemoryKeys::default())
            .err()
            .unwrap()
            .code,
        ErrorCode::KeyUnavailable
    );
    keys.failing.store(true, Ordering::SeqCst);
    assert_eq!(
        Metadata::open(tmp.path(), &keys).err().unwrap().code,
        ErrorCode::KeyUnavailable
    );
    assert_eq!(fs::read(tmp.path().join("root.json")).unwrap(), bytes);
}

#[test]
fn store_initialization_retry_reuses_reserved_identity_after_key_service_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let keys = FailingKeys {
        keys: MemoryKeys::default(),
        failing: AtomicBool::new(true),
    };
    assert_eq!(
        Error::from_anyhow(
            &saucepan::core::Store::create_with_keys(&root, &keys)
                .err()
                .unwrap()
        )
        .code,
        ErrorCode::KeyUnavailable
    );
    let reserved: Uuid = serde_json::from_value(
        serde_json::from_slice::<Value>(
            &fs::read(root.join(".staging/initializing.json")).unwrap(),
        )
        .unwrap()["store"]
            .clone(),
    )
    .unwrap();
    keys.failing.store(false, Ordering::SeqCst);
    let store = saucepan::core::Store::create_with_keys(&root, &keys).unwrap();
    assert_eq!(
        Authenticator::store_hint(&fs::read(root.join("root.json")).unwrap()).unwrap(),
        reserved
    );
    store.register_app("retry".into()).unwrap();
    assert!(!root.join(".staging/initializing.json").exists());
    // Reproduce the final initialization window: root published, marker not yet removed.
    let published = fs::read(root.join("root.json")).unwrap();
    fs::write(
        root.join(".staging/initializing.json"),
        serde_json::to_vec(&reserved).unwrap(),
    )
    .unwrap();
    saucepan::core::Store::create_with_keys(&root, &keys).unwrap();
    assert_eq!(fs::read(root.join("root.json")).unwrap(), published);
    assert!(!root.join(".staging/initializing.json").exists());
}

#[test]
fn unsigned_legacy_roots_are_rejected_unchanged_and_stale_snapshots_cannot_publish() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        unsigned_legacy_roots_are_rejected_unchanged_and_stale_snapshots_cannot_publish_in(mode);
    }
}
fn unsigned_legacy_roots_are_rejected_unchanged_and_stale_snapshots_cannot_publish_in(
    mode: IntegrityMode,
) {
    let tmp = tempfile::tempdir().unwrap();
    root(tmp.path());
    fs::write(tmp.path().join("index/legacy.json"), b"old-format").unwrap();
    let keys = MemoryKeys::default();
    assert_eq!(
        Metadata::open(tmp.path(), &keys).err().unwrap().code,
        ErrorCode::UnsupportedStore
    );
    assert_eq!(
        Metadata::initialize_with_mode(tmp.path(), mode, &keys)
            .err()
            .unwrap()
            .code,
        ErrorCode::UnsupportedStore
    );
    assert!(!tmp.path().join("root.json").exists());
    assert_eq!(
        fs::read(tmp.path().join("index/legacy.json")).unwrap(),
        b"old-format"
    );
    fs::remove_file(tmp.path().join("index/legacy.json")).unwrap();
    let metadata = Metadata::initialize_with_mode(tmp.path(), mode, &keys).unwrap();
    let old = metadata.snapshot().unwrap();
    metadata.publish(&old, vec![], vec![]).unwrap();
    assert_eq!(
        metadata.publish(&old, vec![], vec![]).unwrap_err().code,
        ErrorCode::RevisionConflict
    );
}

#[test]
#[ignore = "explicit native credential-manager integration; creates and deletes one random test entry"]
fn native_keyring_round_trip_and_cleanup() {
    let store = Uuid::new_v4();
    let keys = NativeKeyring;
    assert!(keys.load(store).unwrap().is_none());
    Authenticator::provision(store, &keys).unwrap();
    let result = Authenticator::load(store, &keys);
    let cleanup = keyring::Entry::new("saucepan.metadata-integrity", &store.to_string())
        .unwrap()
        .delete_credential();
    result.unwrap();
    cleanup.unwrap();
    assert!(keys.load(store).unwrap().is_none());
}
fn test_auth(mode: IntegrityMode, store: Uuid, keys: &dyn KeyProvider) -> Authenticator {
    match mode {
        IntegrityMode::Basic => Authenticator::basic(store),
        IntegrityMode::Keyring => Authenticator::provision(store, keys).unwrap(),
    }
}

struct NoCustody;
impl KeyProvider for NoCustody {
    fn load(&self, _: Uuid) -> Result<Option<Secret>> {
        panic!("unexpected key load")
    }
    fn insert(&self, _: Uuid, _: &Secret) -> Result<()> {
        panic!("unexpected key insertion")
    }
}

#[test]
fn basic_initialization_open_and_recovery_never_access_custody() {
    let tmp = tempfile::tempdir().unwrap();
    let store = saucepan::core::Store::create_with_integrity_and_keys(
        tmp.path(),
        IntegrityMode::Basic,
        &NoCustody,
    )
    .unwrap();
    assert_eq!(store.integrity_mode(), IntegrityMode::Basic);
    let bytes = fs::read(tmp.path().join("root.json")).unwrap();
    let reopened = saucepan::core::Store::open_with_keys(tmp.path(), &NoCustody).unwrap();
    assert_eq!(reopened.integrity_mode(), IntegrityMode::Basic);
    assert_eq!(fs::read(tmp.path().join("root.json")).unwrap(), bytes);
    let reserved = Uuid::new_v4();
    let fresh = tempfile::tempdir().unwrap();
    root(fresh.path());
    fs::write(
        fresh.path().join(".staging/initializing.json"),
        serde_json::to_vec(&json!({"store":reserved,"mode":"basic"})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Metadata::initialize_with_mode(fresh.path(), IntegrityMode::Keyring, &NoCustody)
            .err()
            .unwrap()
            .code,
        ErrorCode::InvalidRequest
    );
    let basic =
        Metadata::initialize_with_mode(fresh.path(), IntegrityMode::Basic, &NoCustody).unwrap();
    assert_eq!(basic.authenticator().store(), reserved);
    assert_eq!(
        Metadata::initialize_with_mode(fresh.path(), IntegrityMode::Keyring, &NoCustody)
            .err()
            .unwrap()
            .code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn codecs_reject_cross_mode_records_and_journals_without_effects() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        let tmp = tempfile::tempdir().unwrap();
        root(tmp.path());
        let keys = MemoryKeys::default();
        let metadata = Metadata::initialize_with_mode(tmp.path(), mode, &keys).unwrap();
        let store = metadata.authenticator().store();
        let other = if mode == IntegrityMode::Basic {
            Authenticator::provision(store, &keys).unwrap()
        } else {
            Authenticator::basic(store)
        };
        let identity = Identity::catalog();
        let record = other.sign(identity, 0, &json!({})).unwrap();
        assert!(
            metadata
                .authenticator()
                .verify::<Value>(identity, &record)
                .is_err()
        );
        let pending = tmp.path().join(".staging/metadata-transaction.json");
        fs::write(
            &pending,
            other
                .sign(id(Kind::Transaction, None, None), 1, &json!({}))
                .unwrap(),
        )
        .unwrap();
        let before = fs::read(tmp.path().join("root.json")).unwrap();
        assert_eq!(
            metadata.recover().unwrap_err().code,
            ErrorCode::IntegrityError
        );
        assert_eq!(before, fs::read(tmp.path().join("root.json")).unwrap());
        assert!(pending.exists());
    }
}

#[test]
fn basic_checksum_has_independent_framed_digest_and_rejects_unknown_contracts() {
    use sha2::{Digest, Sha256};
    let store = Uuid::nil();
    let identity = Identity::catalog();
    let auth = Authenticator::basic(store);
    let bytes = auth.sign(identity, 7, &json!("hello")).unwrap();
    let envelope: Value = serde_json::from_slice(&bytes).unwrap();
    let header = format!(
        "[\"saucepan.checksummed-record\",\"basic\",\"{store}\",{{\"kind\":\"catalog\",\"owner\":null,\"member\":null}},7]"
    );
    let mut input = Vec::new();
    for field in [
        b"saucepan/checksummed-record".as_slice(),
        header.as_bytes(),
        b"\"hello\"",
    ] {
        input.extend_from_slice(&(field.len() as u64).to_be_bytes());
        input.extend_from_slice(field);
    }
    let expected: [u8; 32] = Sha256::digest(input).into();
    assert_eq!(envelope["checksum"], json!(expected));
    for field in ["mode", "contract"] {
        let mut changed = envelope.clone();
        changed[field] = json!("unknown");
        assert_eq!(
            Authenticator::root_hint(&serde_json::to_vec(&changed).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::UnsupportedStore
        );
    }
    let oversized = vec![b' '; saucepan::core::claim::security::MAX_RECORD_BYTES + 1];
    assert!(Authenticator::root_hint(&oversized).is_err());
    assert!(auth.verify::<Value>(identity, &oversized).is_err());
}
