mod native_root;
use saucepan::models::{
    policy::{Eviction, Identical},
    source::Source,
    store::{Availability, Request, Selector},
};
use std::fs;
#[test]
fn latest_ten_overrides_claims_and_reacquisition() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::write(&input, b"same").unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("owner".into()).unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.policies.identical = Identical::NewEntry;
    let first = store
        .acquire_for(
            app.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    let lease = store.execution_lease(first.claim.entry_uuid).unwrap();
    for _ in 0..10 {
        store.acquire(&request).unwrap();
    }
    assert_eq!(
        store.output(first.claim.entry_uuid).unwrap().state,
        Availability::Ready
    );
    assert!(
        store
            .reclaim()
            .unwrap()
            .deferred
            .contains_key(&first.claim.entry_uuid)
    );
    drop(lease);
    assert!(
        store
            .reclaim()
            .unwrap()
            .reclaimed
            .contains(&first.claim.entry_uuid)
    );
    assert_eq!(
        store.claims(app.uuid).unwrap()[0].state,
        Availability::Reclaimed
    );
    assert!(store.path(first.claim.entry_uuid).is_err());
    assert!(store.source(source.uuid).is_ok());
    let new = store
        .update_claim(app.uuid, first.claim.uuid, &Default::default())
        .unwrap();
    assert_ne!(new.claim.entry_uuid, first.claim.entry_uuid);
    assert_eq!(
        store.output(first.claim.entry_uuid).unwrap().state,
        Availability::Reclaimed
    );
    request.policies.eviction = Eviction::Latest { keep: 3 };
    let custom = store.acquire(&request).unwrap();
    assert_eq!(custom.reclaim.effective_keep[&custom.entry.item_key], 10);
    request.policies.identical = Identical::Reuse;
    let reuse = store.acquire(&request).unwrap();
    assert_eq!(reuse.entry.uuid, custom.entry.uuid);
    assert_eq!(reuse.entry.sequence, custom.entry.sequence);
}

#[cfg(windows)]
#[test]
fn deletion_failure_is_pending_and_retryable() {
    use std::os::windows::fs::OpenOptionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("file");
    fs::write(&input, b"bytes").unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
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
    let first = store.acquire(&request).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(first.directory.join("file"))
        .unwrap();
    let next = store.acquire(&request).unwrap();
    assert!(next.reclaim.pending.contains_key(&first.entry.uuid));
    assert_eq!(
        store.output(first.entry.uuid).unwrap().state,
        Availability::ReclaimPending
    );
    assert!(store.path(first.entry.uuid).is_err());
    drop(held);
    assert!(
        store
            .reclaim()
            .unwrap()
            .reclaimed
            .contains(&first.entry.uuid)
    );
}
