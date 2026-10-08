use saucepan::{
    core::Store,
    models::{
        policy::{Eviction, Identical},
        source::Source,
        store::{Availability, Request, Selector},
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
    let first = store.lease_claim(app.uuid, claimed.claim.uuid).unwrap();
    let second = store.lease(old).unwrap();
    let updated = store
        .update_claim(app.uuid, claimed.claim.uuid, &Default::default())
        .unwrap();
    let current = store.lease_claim(app.uuid, claimed.claim.uuid).unwrap();
    assert_eq!(current.entry_uuid(), updated.claim.entry_uuid);
    assert_ne!(first.entry_uuid(), current.entry_uuid());
    assert_eq!(first.directory(), claimed.acquired.directory);
    store.release_claim(app.uuid, claimed.claim.uuid).unwrap();
    assert!(store.lease_claim(app.uuid, claimed.claim.uuid).is_err());
    assert!(
        store
            .lease_claim(Uuid::new_v4(), claimed.claim.uuid)
            .is_err()
    );
    assert!(store.lease_claim(app.uuid, Uuid::new_v4()).is_err());
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
    assert!(store.lease(old).is_err());
}

#[test]
fn simultaneous_claim_update_returns_one_consistent_selection() {
    let (_tmp, store, request) = fixture();
    let app = store.register_app("reader".into()).unwrap();
    let claimed = store
        .acquire_for(app.uuid, &request, &Default::default())
        .unwrap();
    let old = store.lease(claimed.claim.entry_uuid).unwrap();
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let updater = scope.spawn(|| {
            start.wait();
            store
                .update_claim(app.uuid, claimed.claim.uuid, &Default::default())
                .unwrap()
        });
        start.wait();
        let selected = store.lease_claim(app.uuid, claimed.claim.uuid).unwrap();
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
