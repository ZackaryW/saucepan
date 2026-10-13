use saucepan::{
    core::{Store, retrieval::RetrieveOptions},
    models::{
        index::Record,
        policy::{Freshness, Identical, RevisionPolicy, UpdatePolicy},
        retrieval::{CachePolicy, Ownership, RetrieveRequest, SourceInput},
        source::{Resolved, Source},
        store::{AcquisitionRequest, AcquisitionState, Selector},
    },
};
use std::{fs, path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8_lossy(&result.stdout).trim().into()
}
fn states(store: &Store) -> Vec<AcquisitionState> {
    store
        .records()
        .unwrap()
        .into_iter()
        .filter_map(|r| match r {
            Record::Acquisition(s) => Some(*s),
            _ => None,
        })
        .collect()
}
fn id(result: &saucepan::core::store::AcquiredResource) -> uuid::Uuid {
    match result.descriptor().ownership {
        Ownership::Managed { uuid } => uuid,
        _ => panic!(),
    }
}
fn setup() -> (
    tempfile::TempDir,
    Store,
    RetrieveRequest,
    AcquisitionRequest,
) {
    let root = tempfile::tempdir().unwrap();
    let origin = root.path().join("origin");
    fs::create_dir(&origin).unwrap();
    git(&origin, &["init", "-b", "main"]);
    fs::create_dir(origin.join("selected")).unwrap();
    fs::write(origin.join("selected/file"), b"first").unwrap();
    fs::write(origin.join("unrelated"), b"one").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "first"]);
    let store = Store::create(&root.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Git {
            origin: origin.to_str().unwrap().into(),
        })
        .unwrap();
    let retrieval = RetrieveRequest {
        source: SourceInput::Registered {
            source_uuid: source.uuid,
        },
        selector: Selector::Ref {
            reference: "main".into(),
        },
        asset: Default::default(),
        cache: CachePolicy::default(),
        verification: Default::default(),
        expected_sha256: None,
    };
    let acquisition = AcquisitionRequest {
        folder: Some("selected".into()),
        update_policy: UpdatePolicy::MaxAge { seconds: 60 },
        revision_policy: Some(RevisionPolicy::None {}),
        ..Default::default()
    };
    (root, store, retrieval, acquisition)
}
fn at(time: u64) -> RetrieveOptions {
    RetrieveOptions {
        test_now: Some(time),
        ..Default::default()
    }
}

#[test]
fn updates_check_inside_source_ttl_and_keep_selected_provenance_and_leased_history() {
    let (root, store, retrieval, acquisition) = setup();
    let first = store
        .acquire_composed_with(&retrieval, &acquisition, &at(100))
        .unwrap();
    let original = first.descriptor().selected_source.clone();
    let origin = root.path().join("origin");
    fs::write(origin.join("unrelated"), b"two").unwrap();
    git(&origin, &["commit", "-am", "unrelated"]);
    let fresh_commit = git(&origin, &["rev-parse", "HEAD"]);
    let cached = store
        .acquire_composed_with(&retrieval, &acquisition, &at(159))
        .unwrap();
    assert_eq!(id(&cached), id(&first));
    assert_eq!(states(&store)[0].last_success, Some(100));
    drop(cached);
    let unchanged = store
        .acquire_composed_with(&retrieval, &acquisition, &at(160))
        .unwrap();
    assert_eq!(id(&unchanged), id(&first));
    assert_eq!(unchanged.descriptor().selected_source, original);
    assert_eq!(
        unchanged.descriptor().checked_source,
        Some(Resolved::Git {
            commit: fresh_commit
        })
    );
    assert!(!unchanged.descriptor().changed);
    assert_eq!(states(&store)[0].last_success, Some(160));
    drop(unchanged);
    fs::write(origin.join("selected/file"), b"new bytes").unwrap();
    git(&origin, &["commit", "-am", "selected"]);
    let changed = store
        .acquire_composed_with(&retrieval, &acquisition, &at(220))
        .unwrap();
    assert_ne!(id(&first), id(&changed));
    assert!(changed.descriptor().changed);
    assert!(
        changed
            .descriptor()
            .reclaim
            .as_ref()
            .unwrap()
            .deferred
            .contains_key(&id(&first))
    );
    let old = id(&first);
    drop(first);
    assert!(store.reclaim().unwrap().reclaimed.contains(&old));
    assert!(changed.path().exists());
    assert_eq!(states(&store).len(), 1);
}

#[test]
fn failed_update_and_stale_fallback_preserve_selection_and_last_success() {
    let (root, store, mut retrieval, acquisition) = setup();
    let first = store
        .acquire_composed_with(&retrieval, &acquisition, &at(100))
        .unwrap();
    let before = serde_json::to_value(states(&store)).unwrap();
    fs::rename(root.path().join("origin"), root.path().join("offline")).unwrap();
    assert!(
        store
            .acquire_composed_with(&retrieval, &acquisition, &at(160))
            .is_err()
    );
    assert_eq!(serde_json::to_value(states(&store)).unwrap(), before);
    retrieval.cache = CachePolicy::Cached {
        freshness: None,
        allow_stale: true,
    };
    let stale = store
        .acquire_composed_with(&retrieval, &acquisition, &at(160))
        .unwrap();
    assert_eq!(id(&first), id(&stale));
    assert!(stale.descriptor().stale);
    assert_eq!(serde_json::to_value(states(&store)).unwrap(), before);
}

#[test]
fn due_cache_only_is_rejected_and_new_entry_keeps_its_independent_meaning() {
    let (_root, store, mut retrieval, mut acquisition) = setup();
    retrieval.cache = CachePolicy::Cached {
        freshness: Some(Freshness::CacheOnly {}),
        allow_stale: false,
    };
    assert!(
        store
            .acquire_composed_with(&retrieval, &acquisition, &at(100))
            .is_err()
    );
    assert!(states(&store).is_empty());
    retrieval.cache = CachePolicy::default();
    acquisition.identical = Identical::NewEntry;
    let first = store
        .acquire_composed_with(&retrieval, &acquisition, &at(100))
        .unwrap();
    let second = store
        .acquire_composed_with(&retrieval, &acquisition, &at(101))
        .unwrap();
    assert_ne!(id(&first), id(&second));
    assert_eq!(
        first.descriptor().selected_source,
        second.descriptor().selected_source
    );
    assert_eq!(states(&store).len(), 1);
    assert_eq!(states(&store)[0].last_success, Some(100));
}

#[test]
fn pinned_updates_stay_pinned_and_concurrent_requests_share_check_state() {
    let (root, store, mut retrieval, mut acquisition) = setup();
    let origin = root.path().join("origin");
    let commit = git(&origin, &["rev-parse", "HEAD"]);
    retrieval.selector = Selector::Commit {
        commit: commit.clone(),
    };
    acquisition.update_policy = UpdatePolicy::MaxAge { seconds: 100 };
    let threads: Vec<_> = (0..3)
        .map(|_| {
            let store = store.clone();
            let retrieval = retrieval.clone();
            let acquisition = acquisition.clone();
            std::thread::spawn(move || {
                let resource = store
                    .acquire_composed_with(&retrieval, &acquisition, &at(100))
                    .unwrap();
                id(&resource)
            })
        })
        .collect();
    let ids: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert!(ids.iter().all(|id| id == &ids[0]));
    assert_eq!(states(&store).len(), 1);
    assert_eq!(states(&store)[0].revision, 1);
    fs::write(origin.join("selected/file"), b"moved").unwrap();
    git(&origin, &["commit", "-am", "move"]);
    let pinned = store
        .acquire_composed_with(&retrieval, &acquisition, &at(200))
        .unwrap();
    assert_eq!(
        pinned.descriptor().selected_source,
        Resolved::Git { commit }
    );
    assert_eq!(fs::read(pinned.path().join("file")).unwrap(), b"first");
}

#[test]
fn changed_policy_has_independent_proof_and_failed_materialization_does_not_advance_it() {
    let (root, store, retrieval, mut acquisition) = setup();
    let first = store
        .acquire_composed_with(&retrieval, &acquisition, &at(100))
        .unwrap();
    acquisition.update_policy = UpdatePolicy::Always {};
    let second = store
        .acquire_composed_with(&retrieval, &acquisition, &at(101))
        .unwrap();
    assert_eq!(id(&first), id(&second));
    assert_eq!(states(&store).len(), 2);
    let before = serde_json::to_value(states(&store)).unwrap();
    let origin = root.path().join("origin");
    fs::rename(origin.join("selected"), origin.join("removed")).unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "remove folder"]);
    assert!(
        store
            .acquire_composed_with(&retrieval, &acquisition, &at(102))
            .is_err()
    );
    assert_eq!(serde_json::to_value(states(&store)).unwrap(), before);
    assert_eq!(fs::read(second.path().join("file")).unwrap(), b"first");
}
