use saucepan::{
    core::Store,
    models::{
        policy::{Eviction, Identical},
        source::Source,
        store::{Availability, Request, Selector, VerificationMode},
    },
};
use std::fs;
use uuid::Uuid;

fn fixture() -> (tempfile::TempDir, Store, Request) {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("data");
    fs::write(&input, b"original").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.policies.identical = Identical::NewEntry;
    request.policies.eviction = Eviction::Latest { keep: 1 };
    (tmp, store, request)
}

#[test]
fn claim_selection_and_multiple_guards_keep_exact_output_alive() {
    let (_tmp, store, request) = fixture();
    let app = store.register_app("reader".into()).unwrap();
    let claimed = store
        .acquire_for(app.uuid, &request, &Default::default())
        .unwrap();
    let old = claimed.claim.entry_uuid;
    let before = serde_json::to_value(store.output(old).unwrap()).unwrap();
    let first = store
        .lease_claim(app.uuid, claimed.claim.uuid, VerificationMode::Metadata)
        .unwrap();
    let second = store.lease(old, VerificationMode::Metadata).unwrap();
    let updated = store
        .update_claim(app.uuid, claimed.claim.uuid, &Default::default())
        .unwrap();
    let current = store
        .lease_claim(app.uuid, claimed.claim.uuid, VerificationMode::Metadata)
        .unwrap();
    assert_eq!(current.entry_uuid(), updated.claim.entry_uuid);
    assert_ne!(first.entry_uuid(), current.entry_uuid());
    assert_eq!(first.directory(), claimed.acquired.directory);
    store.release_claim(app.uuid, claimed.claim.uuid).unwrap();
    assert!(
        store
            .lease_claim(app.uuid, claimed.claim.uuid, VerificationMode::Metadata)
            .is_err()
    );
    assert!(
        store
            .lease_claim(
                Uuid::new_v4(),
                claimed.claim.uuid,
                VerificationMode::Metadata
            )
            .is_err()
    );
    assert!(
        store
            .lease_claim(app.uuid, Uuid::new_v4(), VerificationMode::Metadata)
            .is_err()
    );
    drop(first);
    assert!(store.reclaim().unwrap().deferred.contains_key(&old));
    assert_eq!(
        fs::read(second.directory().join("data")).unwrap(),
        b"original"
    );
    assert_eq!(
        serde_json::to_value(store.output(old).unwrap()).unwrap(),
        before
    );
    drop(second);
    assert!(store.reclaim().unwrap().reclaimed.contains(&old));
    assert_eq!(store.output(old).unwrap().state, Availability::Reclaimed);
    assert!(store.lease(old, VerificationMode::Metadata).is_err());
}

#[test]
fn simultaneous_claim_update_returns_one_consistent_selection() {
    let (_tmp, store, request) = fixture();
    let app = store.register_app("reader".into()).unwrap();
    let claimed = store
        .acquire_for(app.uuid, &request, &Default::default())
        .unwrap();
    let old = store
        .lease(claimed.claim.entry_uuid, VerificationMode::Metadata)
        .unwrap();
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let updater = scope.spawn(|| {
            start.wait();
            store
                .update_claim(app.uuid, claimed.claim.uuid, &Default::default())
                .unwrap()
        });
        start.wait();
        let selected = store
            .lease_claim(app.uuid, claimed.claim.uuid, VerificationMode::Metadata)
            .unwrap();
        let new = updater.join().unwrap();
        assert!([old.entry_uuid(), new.claim.entry_uuid].contains(&selected.entry_uuid()));
        assert_eq!(
            selected.directory(),
            store
                .root()
                .join("store")
                .join(selected.entry_uuid().to_string())
        );
        store.release_claim(app.uuid, claimed.claim.uuid).unwrap();
        assert_eq!(
            fs::read(selected.directory().join("data")).unwrap(),
            b"original"
        );
    });
}

#[test]
fn metadata_and_full_access_native_and_cli_preserve_identity() {
    let (_tmp, store, mut request) = fixture();
    request.policies.eviction = Eviction::Latest { keep: 10 };
    let old = store.acquire(&request).unwrap();
    store.acquire(&request).unwrap();
    let index = store
        .root()
        .join(format!("index/{}/index.json", old.entry.uuid));
    let source = store
        .root()
        .join(format!("source/{}.json", request.source_uuid));
    let before = (fs::read(&index).unwrap(), fs::read(&source).unwrap());
    let resolved = store
        .resolve(old.entry.uuid, VerificationMode::Metadata)
        .unwrap();
    assert_eq!(resolved.directory, old.directory);
    assert_eq!(resolved.verification, VerificationMode::Metadata);
    assert_eq!(
        store.verify(old.entry.uuid).unwrap().verification,
        VerificationMode::Full
    );
    for args in [
        vec!["path"],
        vec!["path", "--verification", "full"],
        vec!["verify"],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(store.root())
            .args(&args)
            .arg(old.entry.uuid.to_string())
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["entry_uuid"], old.entry.uuid.to_string());
        assert_eq!(
            value["verification"],
            if args.len() == 1 && args[0] == "path" {
                "metadata"
            } else {
                "full"
            }
        );
    }
    assert_eq!(
        (fs::read(&index).unwrap(), fs::read(&source).unwrap()),
        before
    );
    let payload = old.directory.join("data");
    let invalid = std::process::Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .args(["path", "--verification", "unknown"])
        .arg(old.entry.uuid.to_string())
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&payload)
            .unwrap();
        assert!(store.path(old.entry.uuid).is_ok());
        assert!(store.verify(old.entry.uuid).is_err());
        drop(held);
    }
    for kind in ["changed", "missing", "extra", "type"] {
        match kind {
            "changed" => fs::write(&payload, b"modified").unwrap(),
            "missing" => fs::remove_file(&payload).unwrap(),
            "extra" => fs::write(old.directory.join("extra"), b"extra").unwrap(),
            "type" => {
                fs::remove_file(&payload).unwrap();
                fs::create_dir(&payload).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(store.path(old.entry.uuid).is_ok());
        assert!(store.verify(old.entry.uuid).is_err());
        assert!(store.execution_lease(old.entry.uuid).is_err());
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(store.root())
            .arg("verify")
            .arg(old.entry.uuid.to_string())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        if kind == "extra" {
            fs::remove_file(old.directory.join("extra")).unwrap();
        }
        if kind == "type" {
            fs::remove_dir(&payload).unwrap();
        }
        fs::write(&payload, b"original").unwrap();
    }
    assert_eq!(fs::read(index).unwrap(), before.0);
}

#[test]
fn metadata_rejects_wrong_identity_kind_state_and_root() {
    let (_tmp, store, request) = fixture();
    let app = store.register_app("wrong kind".into()).unwrap();
    assert!(store.path(app.uuid).is_err());
    assert!(store.path(Uuid::new_v4()).is_err());
    let acquired = store.acquire(&request).unwrap();
    let id = acquired.entry.uuid;
    let index = store.root().join(format!("index/{id}/index.json"));
    let original = fs::read(&index).unwrap();
    let mut entry = acquired.entry.clone();
    entry.uuid = Uuid::new_v4();
    fs::write(
        &index,
        serde_json::to_vec(&saucepan::models::index::Record::Output(Box::new(entry))).unwrap(),
    )
    .unwrap();
    assert!(store.path(id).is_err());
    fs::write(&index, &original).unwrap();
    let moved = acquired.directory.with_extension("saved");
    fs::rename(&acquired.directory, &moved).unwrap();
    assert!(store.path(id).is_err());
    fs::write(&acquired.directory, b"not a directory").unwrap();
    assert!(store.path(id).is_err());
    fs::remove_file(&acquired.directory).unwrap();
    #[cfg(windows)]
    {
        // Junctions require no symlink privilege; the test root owns both endpoints.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&acquired.directory)
            .arg(&moved)
            .output()
            .unwrap();
        assert!(status.status.success(), "{:?}", status);
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&moved, &acquired.directory).unwrap();
    assert!(store.path(id).is_err());
    #[cfg(windows)]
    fs::remove_dir(&acquired.directory).unwrap();
    #[cfg(unix)]
    fs::remove_file(&acquired.directory).unwrap();
    fs::rename(&moved, &acquired.directory).unwrap();
    for state in [Availability::ReclaimPending, Availability::Reclaimed] {
        let mut entry = acquired.entry.clone();
        entry.state = state;
        fs::write(
            &index,
            serde_json::to_vec(&saucepan::models::index::Record::Output(Box::new(entry))).unwrap(),
        )
        .unwrap();
        assert!(store.path(id).is_err());
    }
}
