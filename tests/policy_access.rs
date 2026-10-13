use saucepan::{
    core::{Store, policy::catalog::PolicyCatalog},
    error::{Error, ErrorCode},
    models::{
        claim::EnsureRequest,
        policy::{PolicyRecord, requests::PathAccess},
        source::Source,
        store::{Selector, VerificationMode},
    },
};
use serde_json::json;
use std::{fs, process::Command};
use uuid::Uuid;

fn record(target: &str, mode: &str) -> PolicyRecord {
    serde_json::from_value(
        json!({"uuid":Uuid::new_v4(),"type":"verification_policy","policy_target":target,"mode":mode}),
    )
    .unwrap()
}

#[test]
fn path_reads_resolve_defaults_overrides_and_targets_without_changing_retrieval() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::write(&input, b"original").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("reader".into()).unwrap();
    let result = store
        .ensure_claim(
            app.uuid,
            &EnsureRequest {
                source_uuid: source.uuid,
                scope: "main".to_string().try_into().unwrap(),
                selector: Selector::Local,
                selection: Default::default(),
                context: Default::default(),
                policy_refs: Default::default(),
            },
            &Default::default(),
        )
        .unwrap();
    let full = record("path", "full");
    let metadata = record("path", "metadata");
    let payload = record("payload", "required");
    store
        .register_policies(&[full.clone(), metadata.clone(), payload.clone()])
        .unwrap();
    store
        .adopt_source_policies(source.uuid, 0, vec![full.uuid].try_into().unwrap())
        .unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let lease = session
        .lease_view_referenced(&result.view, &Default::default(), &Default::default())
        .unwrap();
    assert_eq!(lease.resource().verification, VerificationMode::Full);
    drop(lease);
    fs::write(result.acquired.directory.join("input"), b"modified").unwrap();
    let before = fs::read(store.root().join("root.json")).unwrap();
    assert_eq!(
        Error::from_anyhow(
            &store
                .resolve_referenced(
                    result.claim.entry_uuid,
                    &Default::default(),
                    &Default::default()
                )
                .unwrap_err()
        )
        .code,
        ErrorCode::IntegrityError
    );
    let request = PathAccess {
        policy_refs: vec![metadata.uuid].try_into().unwrap(),
    };
    assert_eq!(
        store
            .resolve_referenced(result.claim.entry_uuid, &request, &Default::default())
            .unwrap()
            .verification,
        VerificationMode::Metadata
    );
    let invalid = PathAccess {
        policy_refs: vec![payload.uuid].try_into().unwrap(),
    };
    assert_eq!(
        Error::from_anyhow(
            &store
                .resolve_referenced(result.claim.entry_uuid, &invalid, &Default::default())
                .unwrap_err()
        )
        .code,
        ErrorCode::InvalidRequest
    );
    assert!(
        store
            .resolve_referenced(
                result.claim.entry_uuid,
                &request,
                &PolicyCatalog::new([metadata.clone()]).unwrap()
            )
            .is_err()
    );
    assert_eq!(before, fs::read(store.root().join("root.json")).unwrap());

    let cli = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(store.root())
            .args([
                "--error-format",
                "json",
                "path",
                &result.claim.entry_uuid.to_string(),
            ])
            .args(args)
            .output()
            .unwrap()
    };
    let failed = cli(&[]);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    let passed = cli(&["--policy", &metadata.uuid.to_string()]);
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&passed.stdout).unwrap()["verification"],
        "metadata"
    );
    assert!(cli(&["--verification", "metadata"]).status.success());
    let conflict = cli(&[
        "--policy",
        &metadata.uuid.to_string(),
        "--verification",
        "metadata",
    ]);
    assert!(!conflict.status.success());
    assert!(conflict.stdout.is_empty());
}
