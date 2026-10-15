mod support;
use saucepan::{
    core::{
        Store,
        policy::{catalog::PolicyCatalog, explain, profile},
        retrieval::RetrieveOptions,
    },
    error::{Error, ErrorCode},
    models::{
        claim::request::Ensure,
        policy::{
            PolicyRecord,
            explanation::ExplainRequest,
            profile::{ProfileName, ProfileRequest},
            requests::Retrieval,
        },
        retrieval::SourceInput,
        source::Source,
        store::Selector,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use uuid::Uuid;

fn policy(value: Value) -> PolicyRecord {
    let mut value = value;
    value["uuid"] = json!(Uuid::new_v4());
    serde_json::from_value(value).unwrap()
}

#[test]
fn explanation_requires_recovery_without_replaying_pending_metadata() {
    use saucepan::core::claim::{
        persistence::Metadata,
        security::{Identity, Kind, NativeKeyring},
    };
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "one").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let request: ExplainRequest = serde_json::from_value(json!({"operation":"retrieve","request":{
        "source":{"kind":"registered","source_uuid":source.uuid},"selector":{"type":"local"}
    }}))
    .unwrap();
    let metadata = Metadata::open(store.root(), &NativeKeyring).unwrap();
    let snapshot = metadata.snapshot().unwrap();
    let id = Identity {
        kind: Kind::Source,
        owner: None,
        member: Some(source.uuid),
    };
    assert!(
        metadata
            .publish_observed(
                &snapshot,
                vec![(id, serde_json::to_value(&source).unwrap())],
                vec![],
                |_| { Err(Error::new(ErrorCode::IoError, "stop after journal")) }
            )
            .is_err()
    );
    let pending = store.root().join(".staging/metadata-transaction.json");
    let before = fs::read(&pending).unwrap();
    let root_before = fs::read(store.root().join("root.json")).unwrap();
    assert_eq!(
        Error::from_anyhow(
            &store
                .explain_policies(&request, &PolicyCatalog::default())
                .unwrap_err()
        )
        .code,
        ErrorCode::RecoveryRequired
    );
    let input = tmp.path().join("request.json");
    fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .args(["--error-format", "json", "explain-policies"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["code"],
        "recovery_required"
    );
    assert_eq!(fs::read(&pending).unwrap(), before);
    assert_eq!(
        fs::read(store.root().join("root.json")).unwrap(),
        root_before
    );
}

#[test]
fn portable_explanation_and_cli_do_not_fetch_or_create_a_root() {
    let expanded = profile::expand(&ProfileRequest {
        name: ProfileName::Temporary,
        ..Default::default()
    })
    .unwrap();
    let input: ExplainRequest = serde_json::from_value(json!({"operation":"retrieve","request":{
        "source":{"kind":"inline","source":{"type":"url","url":"http://127.0.0.1:1/must-not-fetch"}},
        "selector":{"type":"url"},"policy_refs":expanded.retrieval_policy_refs }})).unwrap();
    let report = serde_json::to_value(
        explain::portable(
            &input,
            &PolicyCatalog::new(expanded.catalog.clone()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(report["axes"]["cache"]["value"], "temporary");
    assert_eq!(report["axes"]["cache"]["origin"], "request");
    assert_eq!(
        report["axes"]["payload_verification"]["value"],
        "if_available"
    );
    assert_eq!(report["axes"]["payload_verification"]["origin"], "built_in");
    assert_eq!(report["axes"]["revision"]["applicable"], false);
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("request"),
        serde_json::to_vec(&input).unwrap(),
    )
    .unwrap();
    fs::write(
        tmp.path().join("policies"),
        serde_json::to_vec(&expanded.catalog).unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .current_dir(tmp.path())
        .args([
            "--root",
            "missing",
            "--policies",
            "policies",
            "explain-policies",
            "request",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        report
    );
    assert!(!tmp.path().join("missing").exists());
}

#[test]
fn claim_explanation_freezes_adopted_behavior_and_enforces_current_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "one").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let app = store.register_app("app".into()).unwrap();
    let base = profile::expand(&ProfileRequest::default()).unwrap();
    store.register_policies(&base.catalog).unwrap();
    store
        .adopt_application_policies(app.uuid, app.revision, base.combined_policy_refs, None)
        .unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let first = session
        .ensure_referenced(
            &Ensure {
                scope: "project".to_owned().try_into().unwrap(),
                source_uuid: source.uuid,
                selector: Selector::Local,
                selection: Default::default(),
                context: Default::default(),
                policy_refs: Default::default(),
            },
            &Default::default(),
        )
        .unwrap();
    let custom = policy(json!({"type":"revision_policy","history":"latest","keep":3}));
    store
        .register_policies(std::slice::from_ref(&custom))
        .unwrap();
    store
        .adopt_application_policies(
            app.uuid,
            store.application(app.uuid).unwrap().revision,
            vec![custom.uuid].try_into().unwrap(),
            None,
        )
        .unwrap();
    let input = ExplainRequest::Claim {
        app: app.uuid,
        claim: first.claim.uuid,
    };
    let before = fs::read(store.root().join("root.json")).unwrap();
    let report = serde_json::to_value(
        session
            .clone()
            .explain_policies(&input, &PolicyCatalog::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        report["axes"]["revision"]["value"],
        json!({"history":"latest","keep":10})
    );
    assert_eq!(report["axes"]["revision"]["origin"], "adopted_claim");
    assert_eq!(before, fs::read(store.root().join("root.json")).unwrap());
    let deny = policy(json!({"type":"access_policy","scopes":[]}));
    store
        .register_policies(std::slice::from_ref(&deny))
        .unwrap();
    store
        .adopt_application_policies(
            app.uuid,
            store.application(app.uuid).unwrap().revision,
            vec![deny.uuid].try_into().unwrap(),
            None,
        )
        .unwrap();
    let error = session
        .explain_policies(&input, &PolicyCatalog::default())
        .unwrap_err();
    assert_eq!(Error::from_anyhow(&error).code, ErrorCode::AccessDenied);
}

#[test]
fn profile_freshness_changes_origin_checks_and_reuses_unchanged_material() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let server = support::Server::new(move |_, _| {
        count.fetch_add(1, Ordering::SeqCst);
        (200, b"unchanged".to_vec())
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Url {
            url: server.base.clone(),
        })
        .unwrap();
    let managed = profile::expand(&ProfileRequest::default()).unwrap();
    store.register_policies(&managed.catalog).unwrap();
    let mut request = Retrieval {
        source: SourceInput::Registered {
            source_uuid: source.uuid,
        },
        selector: Selector::Url,
        asset: Default::default(),
        policy_refs: managed.retrieval_policy_refs,
        destination: None,
        expected_sha256: None,
    };
    let first = store
        .retrieve_referenced(
            &request,
            &PolicyCatalog::default(),
            &RetrieveOptions {
                test_now: Some(100),
                ..Default::default()
            },
        )
        .unwrap();
    let n = calls.load(Ordering::SeqCst);
    let second = store
        .retrieve_referenced(
            &request,
            &PolicyCatalog::default(),
            &RetrieveOptions {
                test_now: Some(101),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), n);
    assert_eq!(first.source_path(), second.source_path());
    let original_path = first.source_path().to_owned();
    drop(first);
    drop(second);
    let explicit = profile::expand(&ProfileRequest {
        name: ProfileName::ExplicitUpdate,
        ..Default::default()
    })
    .unwrap();
    store.register_policies(&explicit.catalog).unwrap();
    request.policy_refs = explicit.retrieval_policy_refs;
    let third = store
        .retrieve_referenced(
            &request,
            &PolicyCatalog::default(),
            &RetrieveOptions {
                test_now: Some(102),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(calls.load(Ordering::SeqCst) > n);
    assert_eq!(original_path, third.source_path());
}

#[test]
fn explanations_reject_unknown_conflicting_and_corrupt_records() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("absent"), "fixture").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local {
            path: tmp.path().join("absent"),
        })
        .unwrap();
    let profile = profile::expand(&ProfileRequest::default()).unwrap();
    store.register_policies(&profile.catalog).unwrap();
    let input = |ids: Value| -> ExplainRequest {
        serde_json::from_value(json!({"operation":"retrieve","request":{
        "source":{"kind":"registered","source_uuid":source.uuid},"selector":{"type":"local"},"policy_refs":ids}})).unwrap()
    };
    assert_eq!(
        Error::from_anyhow(
            &store
                .explain_policies(&input(json!([Uuid::new_v4()])), &PolicyCatalog::default())
                .unwrap_err()
        )
        .code,
        ErrorCode::InvalidRequest
    );
    let always = policy(json!({"type":"refresh_policy","mode":"always"}));
    store
        .register_policies(std::slice::from_ref(&always))
        .unwrap();
    let mut ids = profile.retrieval_policy_refs.ids().to_vec();
    ids.push(always.uuid);
    assert_eq!(
        Error::from_anyhow(
            &store
                .explain_policies(&input(json!(ids)), &PolicyCatalog::default())
                .unwrap_err()
        )
        .code,
        ErrorCode::PolicyConflict
    );
    let catalog_member = store
        .root()
        .join("policies")
        .join(format!("{}.json", always.uuid));
    // Locate the documented policy file rather than assuming silent repair.
    if catalog_member.exists() {
        fs::write(catalog_member, "broken").unwrap();
        assert_eq!(
            Error::from_anyhow(
                &store
                    .explain_policies(&input(json!([always.uuid])), &PolicyCatalog::default())
                    .unwrap_err()
            )
            .code,
            ErrorCode::IntegrityError
        );
    } else {
        panic!("policy layout changed: update corrupt-record fixture");
    }
}

#[test]
fn profile_history_defers_pins_and_overrides_cannot_escape_constraints() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "0").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: file.clone() })
        .unwrap();
    let expanded = profile::expand(&ProfileRequest::default()).unwrap();
    store.register_policies(&expanded.catalog).unwrap();
    let request = saucepan::models::store::Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policy_refs: expanded.combined_policy_refs,
    };
    let first = store.acquire(&request).unwrap();
    let pin_policy = policy(json!({"type":"pin_policy","retention":"until_released"}));
    store
        .register_policies(std::slice::from_ref(&pin_policy))
        .unwrap();
    store
        .pin_outputs(&saucepan::models::pin::PinRequest {
            registration_key: "reader".to_string().try_into().unwrap(),
            outputs: vec![first.entry.uuid],
            policy_refs: vec![pin_policy.uuid].try_into().unwrap(),
        })
        .unwrap();
    for i in 1..12 {
        fs::write(&file, i.to_string()).unwrap();
        store.acquire(&request).unwrap();
    }
    store.reclaim().unwrap();
    assert!(
        store
            .root()
            .join("store")
            .join(first.entry.uuid.to_string())
            .exists()
    );
    let bounds = policy(json!({"type":"policy_constraints","minimum_keep":5}));
    let three = policy(json!({"type":"revision_policy","history":"latest","keep":3}));
    store
        .register_policies(&[bounds.clone(), three.clone()])
        .unwrap();
    store
        .adopt_source_policies(
            source.uuid,
            source.policy_revision,
            vec![bounds.uuid].try_into().unwrap(),
        )
        .unwrap();
    let custom = profile::expand(&ProfileRequest {
        catalog: vec![three.clone()],
        acquisition_policy_refs: vec![three.uuid].try_into().unwrap(),
        ..Default::default()
    })
    .unwrap();
    let denied = saucepan::models::store::Request {
        policy_refs: custom.combined_policy_refs,
        ..request
    };
    assert_eq!(
        Error::from_anyhow(&store.acquire(&denied).unwrap_err()).code,
        ErrorCode::PolicyConflict
    );
}
