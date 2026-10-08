use saucepan::{
    core::Store,
    models::{
        policy::{Eviction, Identical},
        source::Source,
        store::{Request, Selector},
    },
};
use std::fs;
fn setup() -> (tempfile::TempDir, Store, Request) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("index.json"), "payload").unwrap();
    let store = Store::create(&dir.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    (dir, store, request)
}
#[test]
fn immutable_versions_and_policy_reuse() {
    let (dir, store, mut req) = setup();
    let first = store.acquire(&req).unwrap();
    assert_eq!(
        fs::read(first.directory.join("index.json")).unwrap(),
        b"payload"
    );
    assert_eq!(store.acquire(&req).unwrap().entry.uuid, first.entry.uuid);
    req.policies.identical = Identical::NewEntry;
    let second = store.acquire(&req).unwrap();
    assert_ne!(first.entry.uuid, second.entry.uuid);
    req.policies.identical = Identical::Reuse;
    req.policies.eviction = Eviction::Latest { keep: 3 };
    assert_ne!(store.acquire(&req).unwrap().entry.uuid, first.entry.uuid);
    fs::write(dir.path().join("input/index.json"), "new").unwrap();
    let third = store.acquire(&req).unwrap();
    assert_ne!(third.entry.content_digest, first.entry.content_digest);
    assert_eq!(
        fs::read(first.directory.join("index.json")).unwrap(),
        b"payload"
    );
    fs::write(third.directory.join("index.json"), "tampered").unwrap();
    assert!(store.path(third.entry.uuid).is_ok());
    assert!(store.verify(third.entry.uuid).is_err());
}
#[test]
fn concurrent_reuse_and_failed_selection() {
    let (_dir, store, request) = setup();
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let s = store.clone();
            let r = request.clone();
            std::thread::spawn(move || s.acquire(&r).unwrap().entry.uuid)
        })
        .collect();
    let ids: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert!(ids.iter().all(|id| *id == ids[0]));
    let mut invalid = request;
    invalid.selection.folder = Some("../escape".into());
    assert!(store.acquire(&invalid).is_err());
    assert_eq!(store.records().unwrap().len(), 1);
}
