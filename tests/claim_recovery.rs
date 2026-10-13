mod policy_fixture;
mod support;
use saucepan::{
    core::{Store, claim::security::MemoryKeys, source::AcquireOptions},
    error::{Error, ErrorCode},
    models::{
        claim::{
            EnsureRequest,
            policy::{ReclaimPolicy, ReclaimStrategy},
        },
        policy::Identical,
        source::Source,
        store::{Availability, Selector, VerificationMode},
    },
};
use std::{
    fs,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}
fn request(store: &Store, source_uuid: uuid::Uuid, strategy: ReclaimStrategy) -> EnsureRequest {
    EnsureRequest {
        scope: "a".to_string().try_into().unwrap(),
        source_uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: [("label".into(), "retained".into())].into(),
        policy_refs: policy_fixture::requested(
            store,
            policy_fixture::Overrides {
                revision: Some(saucepan::models::policy::RevisionPolicy::Latest { keep: 1 }),
                reclaim: Some(ReclaimPolicy::ReclaimPolicy { strategy }),
                ..Default::default()
            },
        )
        .unwrap(),
    }
}
fn evict(store: &Store, claimed: &saucepan::models::claim::Claimed, options: &AcquireOptions) {
    let mut pull = policy_fixture::intent(&claimed.claim.request);
    policy_fixture::replace(
        store,
        &mut pull.policy_refs,
        serde_json::json!({"type":"identical_pull_policy","strategy":Identical::NewEntry}),
    )
    .unwrap();
    store.acquire_with(&pull, options).unwrap();
    assert_eq!(
        store.output(claimed.claim.entry_uuid).unwrap().state,
        Availability::Reclaimed
    );
}

#[test]
fn local_recovery_is_explicit_preserves_scope_and_never_activates() {
    for strategy in [
        ReclaimStrategy::ReportUnavailable,
        ReclaimStrategy::RestoreExact,
        ReclaimStrategy::Refresh,
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let store =
            Store::create_with_keys(&tmp.path().join("root"), &MemoryKeys::default()).unwrap();
        let input = tmp.path().join("file");
        fs::write(&input, "original").unwrap();
        let source = store
            .register_source(Source::Local {
                path: input.clone(),
            })
            .unwrap();
        let app = store.register_app("consumer".into()).unwrap().uuid;
        let session = store.session(app, None).unwrap();
        let mut input_request = request(&store, source.uuid, strategy);
        let first = session.ensure(&input_request, &Default::default()).unwrap();
        input_request.scope = "b".to_string().try_into().unwrap();
        let independent = session.ensure(&input_request, &Default::default()).unwrap();
        evict(&store, &first, &Default::default());
        assert_eq!(
            code(
                session
                    .lease_claim(first.claim.uuid, VerificationMode::Full)
                    .err()
                    .unwrap()
            ),
            ErrorCode::ContentUnavailable
        );
        let root_before = fs::read(store.root().join("root.json")).unwrap();
        if strategy == ReclaimStrategy::ReportUnavailable {
            fs::rename(&input, tmp.path().join("unavailable-source")).unwrap();
            assert_eq!(
                code(
                    session
                        .recover(first.claim.uuid, &Default::default())
                        .unwrap_err()
                ),
                ErrorCode::ContentUnavailable
            );
            assert_eq!(
                fs::read(store.root().join("root.json")).unwrap(),
                root_before
            );
            continue;
        }
        if strategy == ReclaimStrategy::Refresh {
            fs::write(&input, "refreshed").unwrap();
        }
        input_request.scope = first.claim.scope.clone();
        let recovered = session.ensure(&input_request, &Default::default()).unwrap();
        assert_eq!(recovered.claim.uuid, first.claim.uuid);
        assert_eq!(recovered.claim.scope, first.claim.scope);
        assert_eq!(recovered.claim.context, first.claim.context);
        assert_ne!(recovered.claim.entry_uuid, first.claim.entry_uuid);
        assert_eq!(
            store.registry(app).unwrap().claims[&independent.claim.uuid].entry_uuid,
            first.claim.entry_uuid
        );
        assert_eq!(
            store.output(first.claim.entry_uuid).unwrap().state,
            Availability::Reclaimed
        );
        assert!(store.application(app).unwrap().active.is_none());
        assert_eq!(
            fs::read(recovered.acquired.directory.join("file")).unwrap(),
            if strategy == ReclaimStrategy::Refresh {
                b"refreshed".as_slice()
            } else {
                b"original".as_slice()
            }
        );
    }
}

#[test]
fn exact_local_recovery_distinguishes_missing_and_changed_bytes_without_redirecting_claim() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_with_keys(&tmp.path().join("root"), &MemoryKeys::default()).unwrap();
    let input = tmp.path().join("file");
    fs::write(&input, "original").unwrap();
    let source = store
        .register_source(Source::Local {
            path: input.clone(),
        })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap().uuid;
    let session = store.session(app, None).unwrap();
    let first = session
        .ensure(
            &request(&store, source.uuid, ReclaimStrategy::RestoreExact),
            &Default::default(),
        )
        .unwrap();
    evict(&store, &first, &Default::default());
    fs::write(&input, "changed").unwrap();
    assert_eq!(
        code(
            session
                .recover(first.claim.uuid, &Default::default())
                .unwrap_err()
        ),
        ErrorCode::IntegrityError
    );
    fs::rename(&input, tmp.path().join("missing")).unwrap();
    assert_eq!(
        code(
            session
                .recover(first.claim.uuid, &Default::default())
                .unwrap_err()
        ),
        ErrorCode::ExactContentUnavailable
    );
    assert_eq!(
        store.registry(app).unwrap().claims[&first.claim.uuid].entry_uuid,
        first.claim.entry_uuid
    );
}

#[test]
fn exact_git_restores_saved_commit_after_branch_moves() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("git");
    fs::create_dir(&input).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&input)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    fs::write(input.join("file"), "original").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "original"]);
    let store = Store::create_with_keys(&tmp.path().join("root"), &MemoryKeys::default()).unwrap();
    let source = store
        .register_source(Source::Git {
            origin: input.to_str().unwrap().into(),
        })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap().uuid;
    let mut request = request(&store, source.uuid, ReclaimStrategy::RestoreExact);
    request.selector = Selector::Ref {
        reference: "main".into(),
    };
    let first = store
        .ensure_claim(app, &request, &Default::default())
        .unwrap();
    evict(&store, &first, &Default::default());
    fs::write(input.join("file"), "new branch").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "new"]);
    let restored = store
        .recover_claim(
            app,
            first.claim.uuid,
            &AcquireOptions {
                force_refresh: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        restored.acquired.entry.resolved,
        first.acquired.entry.resolved
    );
    assert_eq!(restored.claim.request.selector, request.selector);
    assert_eq!(
        fs::read(restored.acquired.directory.join("file")).unwrap(),
        b"original"
    );
}

#[test]
fn remote_exact_uses_saved_url_and_rejects_replacement_while_refresh_honors_ttl() {
    for release in [false, true] {
        let version = Arc::new(AtomicUsize::new(1));
        let v = version.clone();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let server = support::Server::new(move |path, base| {
            let n = v.load(Ordering::SeqCst);
            if path.starts_with("/repos/") {
                h.fetch_add(1, Ordering::SeqCst);
                (200, serde_json::json!({"id":n,"tag_name":format!("v{n}"),"draft":false,"prerelease":false,
                    "assets":[{"id":n,"name":"tool","browser_download_url":format!("{base}/asset/{n}"),"digest":null}]}).to_string().into_bytes())
            } else {
                (200, format!("body-{n}").into_bytes())
            }
        });
        let tmp = tempfile::tempdir().unwrap();
        let store =
            Store::create_with_keys(&tmp.path().join("root"), &MemoryKeys::default()).unwrap();
        let source = store
            .register_source(if release {
                Source::GithubRelease {
                    repo: "owner/tool".into(),
                }
            } else {
                Source::Url {
                    url: format!("{}/asset", server.base),
                }
            })
            .unwrap();
        let app = store.register_app("consumer".into()).unwrap().uuid;
        let mut request = request(&store, source.uuid, ReclaimStrategy::RestoreExact);
        request.selector = if release {
            Selector::LatestStable
        } else {
            Selector::Url
        };
        request.selection.format = saucepan::models::store::Format::Raw;
        request.selection.filename = Some("tool".into());
        request.selection.filter_refs = policy_fixture::definitions(
            &store,
            vec![serde_json::json!({
                "type":"github_release_asset_filter", "platform": null, "asset_patterns":["tool"]
            })],
        )
        .unwrap();
        let mut options = AcquireOptions {
            test_now: Some(100),
            test_github_api: Some(server.base.clone()),
            ..Default::default()
        };
        let first = store.ensure_claim(app, &request, &options).unwrap();
        evict(&store, &first, &options);
        let count = hits.load(Ordering::SeqCst);
        let exact = store
            .recover_claim(app, first.claim.uuid, &options)
            .unwrap();
        assert_ne!(exact.claim.entry_uuid, first.claim.entry_uuid);
        assert_eq!(hits.load(Ordering::SeqCst), count);
        evict(&store, &exact, &options);
        version.store(2, Ordering::SeqCst);
        assert_eq!(
            code(
                store
                    .recover_claim(app, first.claim.uuid, &options)
                    .unwrap_err()
            ),
            ErrorCode::IntegrityError
        );
        assert_eq!(
            store.registry(app).unwrap().claims[&first.claim.uuid].entry_uuid,
            exact.claim.entry_uuid
        );
        request.scope = "refresh".to_string().try_into().unwrap();
        policy_fixture::replace(
            &store,
            &mut request.policy_refs,
            serde_json::json!({"type":"reclaim_policy","strategy":ReclaimStrategy::Refresh}),
        )
        .unwrap();
        let refresh = store.ensure_claim(app, &request, &options).unwrap();
        evict(&store, &refresh, &options);
        options.test_now = Some(86500);
        let refreshed = store
            .recover_claim(app, refresh.claim.uuid, &options)
            .unwrap();
        assert!(refreshed.acquired.update_checked);
        assert_eq!(
            fs::read(refreshed.acquired.directory.join("tool")).unwrap(),
            b"body-2"
        );
    }
}
