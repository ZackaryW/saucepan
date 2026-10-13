mod native_root;
mod policy_fixture;
use saucepan::{
    core::{
        Store,
        claim::{security::MemoryKeys, session::ApplicationRegistration},
    },
    error::{Error, ErrorCode},
    models::{
        claim::{Claimed, EnsureRequest, Scope},
        policy::Policies,
        source::Source,
        store::{Request, Selector, VerificationMode},
    },
};
use std::{fs, process::Command};

fn scope(value: &str) -> Scope {
    value.to_string().try_into().unwrap()
}
fn setup() -> (tempfile::TempDir, Store, uuid::Uuid, EnsureRequest) {
    let tmp = tempfile::tempdir().unwrap();
    let tree = tmp.path().join("input");
    fs::create_dir_all(tree.join("pkg/sub")).unwrap();
    fs::create_dir(tree.join("pkg-other")).unwrap();
    fs::write(tree.join("pkg/sub/file"), "original").unwrap();
    let store = Store::create_with_keys(&tmp.path().join("root"), &MemoryKeys::default()).unwrap();
    let source = store.register_source(Source::Local { path: tree }).unwrap();
    let registered = store
        .register_application(
            &ApplicationRegistration {
                registration_key: scope("app"),
                name: "consumer".into(),
                credentials_required: false,
                policy_refs: policy_fixture::register(
                    &store,
                    Default::default(),
                    Default::default(),
                )
                .unwrap(),
            },
            None,
        )
        .unwrap();
    let request = EnsureRequest {
        scope: scope("project-a"),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: [("project".into(), "a".into())].into(),
        policy_refs: Default::default(),
    };
    (tmp, store, registered.application.uuid, request)
}
fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}

#[test]
fn exact_and_covered_ensure_share_ownership_and_view_lease_follows_the_current_selection() {
    let (tmp, store, app, mut request) = setup();
    let session = store.session(app, None).unwrap();
    let root = session.ensure(&request, &Default::default()).unwrap();
    let again = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(root.claim.uuid, again.claim.uuid);
    assert_eq!(root.claim.entry_uuid, again.claim.entry_uuid);
    assert!(again.acquired.reused);
    request.selection.folder = Some("pkg/sub".into());
    let covered = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(covered.claim.uuid, root.claim.uuid);
    assert_eq!(covered.view.relative.as_deref(), Some("pkg/sub"));
    assert_eq!(
        fs::read(covered.acquired.directory.join("file")).unwrap(),
        b"original"
    );
    assert_eq!(session.claims().unwrap().len(), 1);
    let retained = session
        .lease_view(&covered.view, VerificationMode::Full)
        .unwrap();
    fs::write(tmp.path().join("input/pkg/sub/file"), "updated").unwrap();
    // Ensure is an ownership/cache operation, not an implicit source refresh.
    assert_eq!(
        session
            .ensure(&request, &Default::default())
            .unwrap()
            .claim
            .entry_uuid,
        root.claim.entry_uuid
    );
    let updated = session
        .update(root.claim.uuid, &Default::default())
        .unwrap();
    assert_ne!(updated.claim.entry_uuid, root.claim.entry_uuid);
    let current = session
        .lease_view(&covered.view, VerificationMode::Full)
        .unwrap();
    assert_eq!(current.entry_uuid(), updated.claim.entry_uuid);
    assert_eq!(
        fs::read(current.directory().join("file")).unwrap(),
        b"updated"
    );
    assert_eq!(
        fs::read(retained.directory().join("file")).unwrap(),
        b"original"
    );
    request.selection.folder = None;
    request.scope = scope("project-b");
    let independent = session.ensure(&request, &Default::default()).unwrap();
    assert_ne!(independent.claim.uuid, root.claim.uuid);
    assert_eq!(independent.claim.entry_uuid, updated.claim.entry_uuid);
}

#[test]
fn reverse_overlap_context_changes_missing_projections_and_policy_conflicts_do_not_add_claims() {
    let (_tmp, store, app, mut request) = setup();
    let session = store.session(app, None).unwrap();
    request.selection.folder = Some("pkg".into());
    let selected = session.ensure(&request, &Default::default()).unwrap();
    request.selection.folder = None;
    assert_eq!(
        code(session.ensure(&request, &Default::default()).unwrap_err()),
        ErrorCode::ClaimOverlap
    );
    request.selection.folder = Some("pkg/sub".into());
    request.context.insert("project".into(), "changed".into());
    assert_eq!(
        code(session.ensure(&request, &Default::default()).unwrap_err()),
        ErrorCode::ClaimContextConflict
    );
    request.context = selected.claim.context;
    request.selection.folder = Some("pkg/missing".into());
    assert_eq!(
        code(session.ensure(&request, &Default::default()).unwrap_err()),
        ErrorCode::InvalidRequest
    );
    request.selection.folder = Some("pkg".into());
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"revision_policy","history":"latest","keep":20}),
    )
    .unwrap();
    assert_eq!(
        code(session.ensure(&request, &Default::default()).unwrap_err()),
        ErrorCode::ClaimOverlap
    );
    assert!(
        policy_fixture::replace(
            &store,
            &mut request.policy_refs,
            serde_json::json!({"type":"revision_policy","history":"latest","keep":0})
        )
        .is_err()
    );
    assert_eq!(session.claims().unwrap().len(), 1);
    request.policy_refs = Default::default();
    request.selection.folder = Some("pkg-other".into());
    let disjoint = session.ensure(&request, &Default::default()).unwrap();
    assert_ne!(disjoint.claim.uuid, selected.claim.uuid);
    let mut escape = disjoint.view;
    escape.relative = Some("../pkg".into());
    assert_eq!(
        code(
            session
                .lease_view(&escape, VerificationMode::Metadata)
                .err()
                .unwrap()
        ),
        ErrorCode::InvalidRequest
    );
}

#[test]
fn explicit_new_entry_advances_one_claim_and_a_covered_pull_keeps_the_whole_owning_request() {
    let (_tmp, store, app, mut request) = setup();
    let session = store.session(app, None).unwrap();
    let first = session.ensure(&request, &Default::default()).unwrap();
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"identical_pull_policy","strategy":"new_entry"}),
    )
    .unwrap();
    store
        .adopt_claim_policies(
            app,
            first.claim.uuid,
            store.application(app).unwrap().revision,
            request.policy_refs.clone(),
        )
        .unwrap();
    request.selection.folder = Some("pkg/sub".into());
    let next = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(next.claim.uuid, first.claim.uuid);
    assert_ne!(next.claim.entry_uuid, first.claim.entry_uuid);
    assert!(next.claim.request.selection.folder.is_none());
    assert_eq!(next.view.relative.as_deref(), Some("pkg/sub"));
    assert!(
        store
            .path(next.claim.entry_uuid)
            .unwrap()
            .join("pkg-other")
            .is_dir()
    );
    assert_eq!(session.claims().unwrap().len(), 1);
}

#[test]
fn actual_concurrent_cli_ensures_return_one_claim_and_one_selected_output() {
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let input = tmp.path().join("file");
    fs::write(&input, "shared").unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("process consumers".into()).unwrap();
    let request = tmp.path().join("request.json");
    fs::write(
        &request,
        serde_json::to_vec(&Request {
            adopted: None,
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policies: Policies::default(),
        })
        .unwrap(),
    )
    .unwrap();
    let children: Vec<_> = (0..6)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_saucepan"))
                .arg("--root")
                .arg(store.root())
                .arg("acquire")
                .arg(&request)
                .args(["--app", &app.uuid.to_string(), "--scope", "project"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let results: Vec<Claimed> = children
        .into_iter()
        .map(|child| {
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            serde_json::from_slice(&result.stdout).unwrap()
        })
        .collect();
    for result in &results {
        assert_eq!(result.claim.uuid, results[0].claim.uuid);
        assert_eq!(result.claim.entry_uuid, results[0].claim.entry_uuid);
    }
    assert_eq!(store.registry(app.uuid).unwrap().claims.len(), 1);
}

#[test]
fn concurrent_registration_and_registered_defaults_are_applied_before_source_work() {
    let (_tmp, store, _app, mut request) = setup();
    let ids = std::thread::scope(|threads| {
        let handles: Vec<_> = (0..6)
            .map(|_| {
                threads.spawn(|| {
                    let mut settings = saucepan::models::claim::policy::Registration::default();
                    settings.defaults.output.revision =
                        saucepan::models::policy::RevisionPolicy::Latest { keep: 3 };
                    settings.maximum_keep = 4;
                    let registration = ApplicationRegistration {
                        registration_key: scope("retry-key"),
                        name: "same name".into(),
                        credentials_required: false,
                        policy_refs: policy_fixture::register(&store, Default::default(), settings)
                            .unwrap(),
                    };
                    let result = store.register_application(&registration, None).unwrap();
                    assert!(result.credential.is_none());
                    result.application.uuid
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(ids.iter().all(|id| *id == ids[0]));
    let session = store.session(ids[0], None).unwrap();
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"revision_policy","history":"latest","keep":5}),
    )
    .unwrap();
    let before = fs::read(store.root().join("root.json")).unwrap();
    assert_eq!(
        code(session.ensure(&request, &Default::default()).unwrap_err()),
        ErrorCode::PolicyConflict
    );
    assert_eq!(fs::read(store.root().join("root.json")).unwrap(), before);
    request.policy_refs = Default::default();
    let result = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(
        result.claim.effective.output.revision,
        saucepan::models::policy::RevisionPolicy::Latest { keep: 3 }
    );
    assert!(request.policy_refs.ids().is_empty());
    assert_eq!(
        result
            .claim
            .request
            .adopted
            .as_ref()
            .unwrap()
            .behavior
            .revision,
        saucepan::models::policy::RevisionPolicy::Latest { keep: 3 }
    );
    assert_eq!(
        store
            .source(request.source_uuid)
            .unwrap()
            .refresh
            .ttl_seconds,
        86400
    );
}

#[test]
fn git_coverage_preserves_selection_constraints_and_does_not_refresh_a_moving_branch() {
    let (tmp, store, app, mut request) = setup();
    let input = tmp.path().join("input");
    let git = |args: &[&str]| {
        let result = Command::new("git")
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
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    git(&["add", "."]);
    git(&["commit", "-m", "original"]);
    request.source_uuid = store
        .register_source(Source::Git {
            origin: input.to_str().unwrap().into(),
        })
        .unwrap()
        .uuid;
    request.selector = Selector::Ref {
        reference: "main".into(),
    };
    let session = store.session(app, None).unwrap();
    let first = session.ensure(&request, &Default::default()).unwrap();
    fs::write(input.join("pkg/sub/file"), "new branch").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "new"]);
    request.selection.folder = Some("pkg/sub".into());
    let covered = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(covered.claim.uuid, first.claim.uuid);
    assert_eq!(
        fs::read(covered.acquired.directory.join("file")).unwrap(),
        b"original"
    );
    request.selection.platform = Some(saucepan::models::store::Platform::host());
    let distinct = session.ensure(&request, &Default::default()).unwrap();
    assert_ne!(distinct.claim.uuid, first.claim.uuid);
    request.selection.format = saucepan::models::store::Format::Raw;
    assert!(session.ensure(&request, &Default::default()).is_err());
    assert_eq!(session.claims().unwrap().len(), 2);
}

#[test]
fn vanished_projected_folder_does_not_advance_owner_and_binary_views_require_manifest_members() {
    let (tmp, store, app, mut request) = setup();
    fs::write(tmp.path().join("input/tool"), "tool").unwrap();
    request.selection.bin = vec!["tool".into()];
    let session = store.session(app, None).unwrap();
    let first = session.ensure(&request, &Default::default()).unwrap();
    request.selection.folder = Some("pkg/sub".into());
    assert_eq!(
        code(session.ensure(&request, &Default::default()).unwrap_err()),
        ErrorCode::InvalidRequest
    );
    request.selection.bin.clear();
    request.scope = scope("no-binary");
    request.selection.folder = None;
    let owner = session.ensure(&request, &Default::default()).unwrap();
    fs::rename(
        tmp.path().join("input/pkg/sub"),
        tmp.path().join("input/pkg/renamed"),
    )
    .unwrap();
    request.selection.folder = Some("pkg/sub".into());
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"identical_pull_policy","strategy":"new_entry"}),
    )
    .unwrap();
    assert!(session.ensure(&request, &Default::default()).is_err());
    let registry = store.registry(app).unwrap();
    assert_eq!(
        registry.claims[&owner.claim.uuid].entry_uuid,
        owner.claim.entry_uuid
    );
    assert_eq!(
        registry.claims[&first.claim.uuid].entry_uuid,
        first.claim.entry_uuid
    );
}
