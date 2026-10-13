mod policy_fixture;
mod support;
use saucepan::{
    core::{Store, retrieval::RetrieveOptions},
    models::{
        policy::Freshness,
        retrieval::{Ownership, RetrieveRequest, SourceInput},
        source::Source,
        store::Selector,
    },
};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn request(id: uuid::Uuid, selector: Selector) -> RetrieveRequest {
    RetrieveRequest {
        source: SourceInput::Registered { source_uuid: id },
        selector,
        asset: Default::default(),
        policy_refs: Default::default(),
        destination: None,
        expected_sha256: None,
    }
}
fn id(resource: &saucepan::core::retrieval::RetrievedResource) -> uuid::Uuid {
    let Ownership::Managed { uuid } = resource.descriptor().ownership else {
        panic!("managed")
    };
    uuid
}

#[test]
fn cached_retrieval_inherits_configured_freshness_and_defers_live_purge() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let server = support::Server::new(move |_, _| {
        count.fetch_add(1, Ordering::SeqCst);
        (200, b"payload".to_vec())
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Url {
            url: server.base.clone(),
        })
        .unwrap();
    store
        .configure_source_freshness(source.uuid, Freshness::MaxAge { seconds: 60 })
        .unwrap();
    let req = request(source.uuid, Selector::Url);
    let mut options = RetrieveOptions {
        test_now: Some(100),
        ..Default::default()
    };
    let first = store.retrieve_with(&req, &options).unwrap();
    let first_id = id(&first);
    store.reclaim().unwrap();
    assert!(
        first.source_path().is_file(),
        "output reclaim cannot evict source material"
    );
    let initial_calls = calls.load(Ordering::SeqCst);
    assert_eq!(initial_calls, 2);
    options.test_now = Some(159);
    let second = store.retrieve_with(&req, &options).unwrap();
    assert_eq!(id(&second), first_id);
    assert_eq!(calls.load(Ordering::SeqCst), initial_calls);
    assert!(!store.purge_retrieval(first_id).unwrap());
    let path = first.source_path().to_owned();
    drop(first);
    drop(second);
    options.test_now = Some(160);
    let refreshed = store.retrieve_with(&req, &options).unwrap();
    assert_eq!(id(&refreshed), first_id);
    assert_eq!(refreshed.descriptor().checked_at, Some(160));
    assert_eq!(calls.load(Ordering::SeqCst), initial_calls + 2);
    drop(refreshed);
    assert!(store.purge_retrieval(first_id).unwrap());
    assert!(!path.exists());
    assert!(store.source(source.uuid).is_ok());
    let mut offline = req;
    policy_fixture::refresh(&store, &mut offline, Freshness::CacheOnly {}, false);
    assert!(store.retrieve_with(&offline, &options).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), initial_calls + 2);
    assert_eq!(fs::read_dir(store.root().join("store")).unwrap().count(), 0);
}

#[test]
fn cache_hits_recheck_stronger_verification_and_refuse_corruption() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::write(&input, b"abc").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut req = request(source.uuid, Selector::Local);
    let first = store.retrieve(&req).unwrap();
    let uuid = id(&first);
    let path = first.source_path().to_owned();
    drop(first);
    policy_fixture::refresh(&store, &mut req, Freshness::CacheOnly {}, false);
    policy_fixture::replace(
        &store,
        &mut req.policy_refs,
        serde_json::json!({"type":"verification_policy","policy_target":"payload","mode":"required"}),
    )
    .unwrap();
    req.expected_sha256 = Some(saucepan::utils::tree::digest(&b"abc"[..]).unwrap());
    let checked = store.retrieve(&req).unwrap();
    assert_eq!(id(&checked), uuid);
    assert_eq!(
        checked.descriptor().verification.expected_sha256,
        req.expected_sha256
    );
    drop(checked);
    fs::write(&path, b"corrupt").unwrap();
    assert!(store.retrieve(&req).is_err());
    assert_eq!(fs::read(path).unwrap(), b"corrupt");
}

#[test]
fn platform_asset_retrieval_does_not_extend_shared_discovery_freshness() {
    let discoveries = Arc::new(AtomicUsize::new(0));
    let count = discoveries.clone();
    let server = support::Server::new(move |path, base| {
        if path.starts_with("/repos/") {
            count.fetch_add(1, Ordering::SeqCst);
            (200, serde_json::json!({"id":1,"tag_name":"v1","draft":false,"prerelease":false,"assets":[{"id":2,"name":"windows.zip","browser_download_url":format!("{base}/windows"),"digest":null},{"id":3,"name":"linux.zip","browser_download_url":format!("{base}/linux"),"digest":null}]}).to_string().into_bytes())
        } else {
            (200, path.as_bytes().to_vec())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/repo".into(),
        })
        .unwrap();
    let mut req = request(source.uuid, Selector::LatestStable);
    policy_fixture::refresh(&store, &mut req, Freshness::MaxAge { seconds: 60 }, false);
    req.asset.filter_refs = policy_fixture::definitions(&store, vec![serde_json::json!({"type":"github_release_asset_filter","platform":null,"asset_patterns":["windows.zip"]})]).unwrap();
    let mut options = RetrieveOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
    };
    drop(store.retrieve_with(&req, &options).unwrap());
    options.test_now = Some(150);
    req.asset.filter_refs = policy_fixture::definitions(&store, vec![serde_json::json!({"type":"github_release_asset_filter","platform":null,"asset_patterns":["linux.zip"]})]).unwrap();
    let linux = store.retrieve_with(&req, &options).unwrap();
    assert_eq!(linux.descriptor().checked_at, Some(100));
    assert_eq!(discoveries.load(Ordering::SeqCst), 1);
    drop(linux);
    options.test_now = Some(160);
    drop(store.retrieve_with(&req, &options).unwrap());
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
    assert!(
        store
            .source(source.uuid)
            .unwrap()
            .retrieval_freshness
            .is_none(),
        "per-call override must not configure the source"
    );
}

#[test]
fn failed_refresh_keeps_last_success_and_explicit_stale_evidence() {
    use std::sync::atomic::AtomicBool;
    let failing = Arc::new(AtomicBool::new(false));
    let flag = failing.clone();
    let server = support::Server::new(move |_, _| {
        if flag.load(Ordering::SeqCst) {
            (503, b"unavailable".to_vec())
        } else {
            (200, b"bytes".to_vec())
        }
    });
    let root = tempfile::tempdir().unwrap();
    let store = Store::create(&root.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Url {
            url: server.base.clone(),
        })
        .unwrap();
    let mut req = request(source.uuid, Selector::Url);
    policy_fixture::refresh(&store, &mut req, Freshness::Always {}, false);
    let first = store
        .retrieve_with(
            &req,
            &RetrieveOptions {
                test_now: Some(100),
                ..Default::default()
            },
        )
        .unwrap();
    let uuid = id(&first);
    drop(first);
    failing.store(true, Ordering::SeqCst);
    let options = RetrieveOptions {
        test_now: Some(200),
        ..Default::default()
    };
    assert!(store.retrieve_with(&req, &options).is_err());
    assert_eq!(store.retrieval_record(uuid).unwrap().checked_at, 100);
    policy_fixture::refresh(&store, &mut req, Freshness::Always {}, true);
    let stale = store.retrieve_with(&req, &options).unwrap();
    assert!(stale.descriptor().stale);
    assert_eq!(stale.descriptor().checked_at, Some(100));
    assert_eq!(
        store
            .source(source.uuid)
            .unwrap()
            .discovery
            .values()
            .next()
            .unwrap()
            .checked_at,
        100
    );
}

#[test]
fn concurrent_configuration_is_not_overwritten_by_inflight_retrieval() {
    use std::{sync::mpsc, time::Duration};
    let (entered, waiting) = mpsc::sync_channel(1);
    let (resume, continued) = mpsc::sync_channel(1);
    let calls = AtomicUsize::new(0);
    let server = support::Server::new(move |_, _| {
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            entered.send(()).unwrap();
            continued.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        (200, b"payload".to_vec())
    });
    let root = tempfile::tempdir().unwrap();
    let store = Store::create(&root.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Url {
            url: server.base.clone(),
        })
        .unwrap();
    let reader = store.clone();
    let retrieving = std::thread::spawn(move || {
        drop(
            reader
                .retrieve(&request(source.uuid, Selector::Url))
                .unwrap(),
        );
    });
    waiting.recv_timeout(Duration::from_secs(10)).unwrap();
    let writer = store.clone();
    let (configured, completed) = mpsc::sync_channel(1);
    let configuring = std::thread::spawn(move || {
        writer
            .configure_source_freshness(source.uuid, Freshness::Always {})
            .unwrap();
        configured.send(()).unwrap();
    });
    assert!(completed.try_recv().is_err());
    resume.send(()).unwrap();
    retrieving.join().unwrap();
    configuring.join().unwrap();
    assert_eq!(
        store.source(source.uuid).unwrap().retrieval_freshness,
        Some(Freshness::Always {})
    );
}
