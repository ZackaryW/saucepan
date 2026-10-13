mod policy_fixture;
mod support;
use saucepan::{
    core::{
        Store,
        retrieval::{RetrieveOptions, Retriever},
    },
    models::{
        retrieval::{MaterialKind, Ownership, RetrieveRequest, SourceInput},
        source::{Resolved, Source},
        store::Selector,
    },
};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn request(source: Source, selector: Selector) -> RetrieveRequest {
    RetrieveRequest {
        source: SourceInput::Inline { source },
        selector,
        asset: Default::default(),
        policy_refs: policy_fixture::fresh_refs(vec![
            serde_json::json!({"type":"cache_policy","mode":"temporary"}),
            serde_json::json!({"type":"verification_policy","policy_target":"payload","mode":"none"}),
        ]),
        destination: None,
        expected_sha256: None,
    }
}

#[test]
fn root_free_child() {
    if let Some(path) = std::env::var_os("SAUCEPAN_TEST_FRESH_INPUT") {
        let source = Source::Local { path: path.into() };
        let result = retrieve(&request(source, Selector::Local)).unwrap();
        assert_eq!(fs::read(result.source_path()).unwrap(), b"root free");
    }
}

#[test]
fn inline_retrieval_needs_neither_home_nor_store() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input");
    fs::write(&input, b"root free").unwrap();
    assert!(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "root_free_child"])
            .env("SAUCEPAN_TEST_FRESH_INPUT", &input)
            .env_remove("HOME")
            .env_remove("USERPROFILE")
            .current_dir(root.path())
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn fresh_http_bypasses_an_existing_registered_source_and_preserves_original_bytes() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let server = support::Server::new(move |_, _| {
        counted.fetch_add(1, Ordering::SeqCst);
        (200, b"original archive bytes".to_vec())
    });
    let root = tempfile::tempdir().unwrap();
    let store = Store::create(&root.path().join("root")).unwrap();
    let source = Source::Url {
        url: format!("{}/archive.zip", server.base),
    };
    store.register_source(source.clone()).unwrap();
    let before = saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap();
    let mut req = request(source, Selector::Url);
    let first = retrieve(&req).unwrap();
    let path = first.source_path().to_owned();
    assert_eq!(fs::read(&path).unwrap(), b"original archive bytes");
    assert_eq!(first.descriptor().kind, MaterialKind::File);
    assert_eq!(first.descriptor().verification.expected_sha256, None);
    assert_eq!(first.descriptor().verification.content_sha256, None);
    let second = retrieve(&req).unwrap();
    assert_ne!(path, second.source_path());
    drop(first);
    assert!(!path.exists());
    let destination = root.path().join("caller.zip");
    req.destination = Some(destination.clone());
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"no_store"}),
    );
    let caller = retrieve(&req).unwrap();
    assert_eq!(caller.descriptor().ownership, Ownership::Caller);
    drop(caller);
    assert_eq!(fs::read(&destination).unwrap(), b"original archive bytes");
    assert_eq!(hits.load(Ordering::SeqCst), 3);
    assert!(retrieve(&req).is_err());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        3,
        "occupied destination must fail before HTTP"
    );
    assert_eq!(
        before,
        saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap()
    );
    drop(server);
    req.destination = None;
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"temporary"}),
    );
    assert!(
        retrieve(&req).is_err(),
        "a previous fresh response is not fallback cache"
    );
}

#[test]
fn local_capture_is_stable_and_caller_owned_delivery_is_disjoint() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("file"), b"one").unwrap();
    let mut req = request(
        Source::Local {
            path: source.clone(),
        },
        Selector::Local,
    );
    let capture = retrieve(&req).unwrap();
    fs::write(source.join("file"), b"two").unwrap();
    assert_eq!(
        fs::read(capture.source_path().join("file")).unwrap(),
        b"one"
    );
    req.destination = Some(source.join("inside"));
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"no_store"}),
    );
    assert!(retrieve(&req).is_err());
    assert!(!source.join("inside").exists());
    req.destination = Some(root.path().join("copy"));
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"no_store"}),
    );
    let copy = retrieve(&req).unwrap();
    assert_eq!(fs::read(copy.source_path().join("file")).unwrap(), b"two");
    drop(copy);
    assert!(root.path().join("copy/file").exists());
}

#[test]
fn expected_digest_checks_are_optional_but_never_fabricated() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("file");
    fs::write(&source, b"abc").unwrap();
    let mut req = request(Source::Local { path: source }, Selector::Local);
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"verification_policy","policy_target":"payload","mode":"required"}),
    );
    assert!(retrieve(&req).is_err());
    req.expected_sha256 = Some(saucepan::utils::tree::digest(&b"abc"[..]).unwrap());
    let checked = retrieve(&req).unwrap();
    assert_eq!(
        checked.descriptor().verification.expected_sha256,
        req.expected_sha256
    );
    req.destination = Some(root.path().join("bad"));
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"no_store"}),
    );
    req.expected_sha256 = Some("0".repeat(64));
    assert!(retrieve(&req).is_err());
    assert!(!root.path().join("bad").exists());
}

#[test]
fn release_retrieval_selects_original_asset_and_checks_upstream_digest() {
    let digest = saucepan::utils::tree::digest(&b"payload"[..]).unwrap();
    let expected = digest.clone();
    let server = support::Server::new(move |path, base| {
        if path.starts_with("/repos/") {
            (200, serde_json::json!({"id":1,"tag_name":"v1","draft":false,"prerelease":false,"assets":[{"id":2,"name":"tool.zip","browser_download_url":format!("{base}/asset"),"digest":format!("sha256:{digest}")}]}).to_string().into_bytes())
        } else {
            (200, b"payload".to_vec())
        }
    });
    let mut req = request(
        Source::GithubRelease {
            repo: "owner/repo".into(),
        },
        Selector::LatestStable,
    );
    req.asset.filter_refs = policy_fixture::fresh_refs(vec![
        serde_json::json!({"type":"github_release_asset_filter","platform":null,"asset_patterns":["*.zip"]}),
    ]);
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"verification_policy","policy_target":"payload","mode":"required"}),
    );
    let result = retrieve_with(
        &req,
        &RetrieveOptions {
            test_github_api: Some(server.base.clone()),
            test_now: Some(100),
        },
    )
    .unwrap();
    assert_eq!(fs::read(result.source_path()).unwrap(), b"payload");
    assert_eq!(
        result.descriptor().verification.expected_sha256,
        Some(expected)
    );
    assert_eq!(result.descriptor().checked_at, Some(100));
}

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().into()
}

#[test]
fn git_retrieval_is_a_fresh_repository_with_exact_provenance() {
    let root = tempfile::tempdir().unwrap();
    let origin = root.path().join("origin");
    fs::create_dir(&origin).unwrap();
    git(&origin, &["init", "-b", "main"]);
    fs::write(origin.join("file"), b"one").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "first"]);
    let first_commit = git(&origin, &["rev-parse", "HEAD"]);
    let mut req = request(
        Source::Git {
            origin: origin.to_str().unwrap().into(),
        },
        Selector::Ref {
            reference: "main".into(),
        },
    );
    let first = retrieve(&req).unwrap();
    assert_eq!(
        first.descriptor().resolved,
        Resolved::Git {
            commit: first_commit.clone()
        }
    );
    assert_eq!(
        git(first.source_path(), &["rev-parse", "--is-bare-repository"]),
        "true"
    );
    fs::write(origin.join("file"), b"two").unwrap();
    git(&origin, &["commit", "-am", "second"]);
    req.destination = Some(root.path().join("repository"));
    policy_fixture::fresh_replace(
        &mut req.policy_refs,
        serde_json::json!({"type":"cache_policy","mode":"no_store"}),
    );
    let second = retrieve(&req).unwrap();
    assert_ne!(first.descriptor().resolved, second.descriptor().resolved);
    assert_eq!(
        git(
            first.source_path(),
            &["show", &format!("{first_commit}:file")]
        ),
        "one"
    );
    let owned_path = second.source_path().to_owned();
    drop(second);
    assert!(owned_path.exists());
    assert!(first.descriptor().verification.git_objects_validated);
}

#[test]
fn managed_git_cache_only_reads_survive_missing_origin_and_purge_keeps_objects() {
    use saucepan::models::policy::Freshness;
    let root = tempfile::tempdir().unwrap();
    let origin = root.path().join("origin");
    fs::create_dir(&origin).unwrap();
    git(&origin, &["init", "-b", "main"]);
    fs::write(origin.join("file"), b"pinned").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "fixture"]);
    let store = Store::create(&root.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Git {
            origin: origin.to_str().unwrap().into(),
        })
        .unwrap();
    let mut req = request(
        source.source,
        Selector::Ref {
            reference: "main".into(),
        },
    );
    req.source = SourceInput::Registered {
        source_uuid: source.uuid,
    };
    req.policy_refs = Default::default();
    req.destination = None;
    let cached = store.retrieve(&req).unwrap();
    let Ownership::Managed { uuid } = cached.descriptor().ownership else {
        panic!("managed")
    };
    let path = cached.source_path().to_owned();
    drop(cached);
    fs::rename(&origin, root.path().join("offline-origin")).unwrap();
    policy_fixture::refresh(&store, &mut req, Freshness::CacheOnly {}, false);
    let offline = store.retrieve(&req).unwrap();
    assert_eq!(offline.source_path(), path);
    assert!(!store.purge_retrieval(uuid).unwrap());
    let lease = store.lease_retrieval(uuid).unwrap();
    drop(offline);
    drop(lease);
    assert!(store.purge_retrieval(uuid).unwrap());
    assert!(path.exists(), "purge must not prune shared Git data");
    assert!(store.source_path(uuid).is_err());
}

fn retrieve(
    input: &RetrieveRequest,
) -> anyhow::Result<saucepan::core::retrieval::RetrievedResource> {
    Retriever::retrieve(input, &policy_fixture::fresh_catalog())
}
fn retrieve_with(
    input: &RetrieveRequest,
    options: &saucepan::core::retrieval::RetrieveOptions,
) -> anyhow::Result<saucepan::core::retrieval::RetrievedResource> {
    Retriever::retrieve_referenced(input, &policy_fixture::fresh_catalog(), options)
}
