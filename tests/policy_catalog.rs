use saucepan::{
    core::{
        Store,
        claim::{
            persistence::{Metadata, Publication},
            security::{Identity, Kind, MemoryKeys},
        },
    },
    error::{Error, ErrorCode},
    models::{claim::security::IntegrityMode, policy::PolicyRecord},
};
use serde_json::json;
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};
use uuid::Uuid;

fn revision(uuid: Uuid, keep: usize) -> PolicyRecord {
    serde_json::from_value(
        json!({"uuid":uuid,"type":"revision_policy","history":"latest","keep":keep}),
    )
    .unwrap()
}
fn identity(uuid: Uuid) -> Identity {
    Identity {
        kind: Kind::Policy,
        owner: None,
        member: Some(uuid),
    }
}
fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}

#[test]
fn registration_is_atomic_immutable_and_retry_does_not_advance_catalog() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        let temp = tempfile::tempdir().unwrap();
        let keys = MemoryKeys::default();
        let root = temp.path().join("root");
        let store = Store::create_with_integrity_and_keys(&root, mode, &keys).unwrap();
        let policy = revision(Uuid::new_v4(), 3);
        store
            .register_policies(std::slice::from_ref(&policy))
            .unwrap();
        let before = fs::read(root.join("root.json")).unwrap();
        store
            .register_policies(std::slice::from_ref(&policy))
            .unwrap();
        assert_eq!(fs::read(root.join("root.json")).unwrap(), before);
        let fresh = revision(Uuid::new_v4(), 5);
        assert_eq!(
            code(
                store
                    .register_policies(&[fresh.clone(), revision(policy.uuid, 8)])
                    .unwrap_err()
            ),
            ErrorCode::PolicyConflict
        );
        assert_eq!(
            code(store.policy(fresh.uuid).unwrap_err()),
            ErrorCode::InvalidRequest
        );
        assert_eq!(store.policy(policy.uuid).unwrap(), policy);
        assert_eq!(fs::read(root.join("root.json")).unwrap(), before);
        assert_eq!(
            Store::open_with_keys(&root, &keys)
                .unwrap()
                .policy(policy.uuid)
                .unwrap(),
            policy
        );
        assert!(
            fs::read_to_string(root.join(format!("policies/{}.json", policy.uuid)))
                .unwrap()
                .contains("revision_policy")
        );
    }
}

#[test]
fn expected_missing_and_tampered_policies_are_integrity_failures() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        for remove in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let keys = MemoryKeys::default();
            let root = temp.path().join("root");
            let store = Store::create_with_integrity_and_keys(&root, mode, &keys).unwrap();
            let policy = revision(Uuid::new_v4(), 3);
            store
                .register_policies(std::slice::from_ref(&policy))
                .unwrap();
            let path = root.join(format!("policies/{}.json", policy.uuid));
            if remove {
                fs::remove_file(path).unwrap();
            } else {
                let mut envelope: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                envelope["payload"]["keep"] = json!(9);
                fs::write(path, serde_json::to_vec(&envelope).unwrap()).unwrap();
            }
            assert_eq!(
                code(store.policy(policy.uuid).unwrap_err()),
                ErrorCode::IntegrityError
            );
        }
    }
}

#[test]
fn policy_journal_recovers_and_low_level_publication_cannot_replace_or_delete() {
    for mode in [IntegrityMode::Basic, IntegrityMode::Keyring] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let keys = MemoryKeys::default();
        Store::create_with_integrity_and_keys(&root, mode, &keys).unwrap();
        let metadata = Metadata::open(&root, &keys).unwrap();
        let policy = revision(Uuid::new_v4(), 3);
        let snapshot = metadata.snapshot().unwrap();
        let result = metadata.publish_observed(
            &snapshot,
            vec![(
                identity(policy.uuid),
                serde_json::to_value(&policy).unwrap(),
            )],
            vec![],
            |event| {
                if matches!(event, Publication::Journal) {
                    Err(Error::new(ErrorCode::IoError, "injected stop"))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err());
        let store = Store::open_with_keys(&root, &keys).unwrap();
        assert_eq!(store.policy(policy.uuid).unwrap(), policy);
        let snapshot = metadata.snapshot().unwrap();
        assert_eq!(
            metadata
                .publish(
                    &snapshot,
                    vec![(
                        identity(policy.uuid),
                        serde_json::to_value(revision(policy.uuid, 5)).unwrap()
                    )],
                    vec![]
                )
                .unwrap_err()
                .code,
            ErrorCode::PolicyConflict
        );
        assert_eq!(
            metadata
                .publish(&snapshot, vec![], vec![identity(policy.uuid)])
                .unwrap_err()
                .code,
            ErrorCode::PolicyConflict
        );
        assert_eq!(store.policy(policy.uuid).unwrap(), policy);
    }
}

#[test]
fn concurrent_conflicting_registration_publishes_one_definition() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    Store::create(&root).unwrap();
    let uuid = Uuid::new_v4();
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = [3, 5]
        .into_iter()
        .map(|keep| {
            let barrier = barrier.clone();
            let root = root.clone();
            thread::spawn(move || {
                let store = Store::open(&root).unwrap();
                barrier.wait();
                store
                    .register_policies(&[revision(uuid, keep)])
                    .map_err(code)
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(ErrorCode::PolicyConflict)))
            .count(),
        1
    );
}
