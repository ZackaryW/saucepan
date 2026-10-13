use saucepan::{core::Store, models::policy::PolicyRecord};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use uuid::Uuid;
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(root)
        .args(["--error-format", "json"])
        .args(args)
        .output()
        .unwrap()
}
fn value(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn error(output: Output, code: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], code);
}
#[test]
fn cli_registration_lookup_and_explicit_adoption_use_verified_catalogs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    value(run(&root, &["init"]));
    let definition: PolicyRecord = serde_json::from_value(json!({"uuid":Uuid::new_v4(),
        "type":"revision_policy","history":"latest","keep":3}))
    .unwrap();
    let file = temp.path().join("policies.json");
    fs::write(
        &file,
        serde_json::to_vec(&vec![definition.clone()]).unwrap(),
    )
    .unwrap();
    value(run(&root, &["register-policies", file.to_str().unwrap()]));
    let id = definition.uuid.to_string();
    let before = fs::read(root.join("root.json")).unwrap();
    assert_eq!(
        value(run(&root, &["policy", &id])),
        serde_json::to_value(&definition).unwrap()
    );
    assert_eq!(before, fs::read(root.join("root.json")).unwrap());
    error(run(&root, &["policy", "malformed"]), "invalid_request");
    error(
        run(&root, &["policy", &Uuid::new_v4().to_string()]),
        "invalid_request",
    );
    let store = Store::open(&root).unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let app_id = app.uuid.to_string();
    error(
        run(&root, &["--as-app", &app_id, "policy", &id]),
        "access_denied",
    );
    let adopted = value(run(
        &root,
        &[
            "adopt-app-policies",
            &app_id,
            "--expected-revision",
            &app.revision.to_string(),
            "--policy",
            &id,
        ],
    ));
    assert_eq!(adopted["policy_refs"], json!([id]));
    assert_eq!(
        value(run(&root, &["--as-app", &app_id, "policy", &id])),
        serde_json::to_value(&definition).unwrap()
    );
    error(
        run(
            &root,
            &[
                "adopt-app-policies",
                &app_id,
                "--expected-revision",
                &app.revision.to_string(),
                "--policy",
                &id,
            ],
        ),
        "revision_conflict",
    );
    error(
        run(
            &root,
            &[
                "--as-app",
                &app_id,
                "register-policies",
                file.to_str().unwrap(),
            ],
        ),
        "access_denied",
    );
    // Catalog membership distinguishes absent user IDs from missing expected files.
    fs::remove_file(root.join(format!("policies/{id}.json"))).unwrap();
    error(run(&root, &["policy", &id]), "integrity_error");
}

#[test]
fn policy_lookup_uses_default_home_root_without_an_application() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::create(&temp.path().join(".saucepan")).unwrap();
    let record: PolicyRecord = serde_json::from_value(json!({"uuid":Uuid::new_v4(),
        "type":"refresh_policy","mode":"always"}))
    .unwrap();
    store
        .register_policies(std::slice::from_ref(&record))
        .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .env("HOME", temp.path())
        .env("USERPROFILE", temp.path())
        .args(["policy", &record.uuid.to_string()])
        .output()
        .unwrap();
    assert_eq!(value(result), serde_json::to_value(record).unwrap());
}

#[test]
fn policy_guide_catalog_registers_and_exact_lookup_matches_the_documented_definition() {
    let guide = include_str!("../docs/policy-resources.md");
    let json = guide
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let records: Vec<PolicyRecord> = serde_json::from_str(json).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    value(run(&root, &["init"]));
    let path = temp.path().join("policies.json");
    fs::write(&path, json).unwrap();
    value(run(&root, &["register-policies", path.to_str().unwrap()]));
    let expected = records
        .iter()
        .find(|r| r.uuid.to_string() == "10000000-0000-4000-8000-000000000003")
        .unwrap();
    assert_eq!(
        value(run(&root, &["policy", &expected.uuid.to_string()])),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn application_payloads_reject_embedded_authority_and_persist_only_references() {
    use saucepan::core::claim::session::ApplicationRegistration;
    for field in ["access", "policies"] {
        let mut input = json!({"registration_key":"app", "name":"app", "policy_refs":[]});
        input[field] = json!({});
        assert!(serde_json::from_value::<ApplicationRegistration>(input).is_err());
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let store = Store::create(&root).unwrap();
    let file = temp.path().join("application.json");
    fs::write(
        &file,
        r#"{"registration_key":"app","name":"app","policy_refs":[]}"#,
    )
    .unwrap();
    let app = value(run(
        &root,
        &["register-application", file.to_str().unwrap()],
    ));
    let id: Uuid = serde_json::from_value(app["uuid"].clone()).unwrap();
    let registry: Value =
        serde_json::from_slice(&fs::read(root.join(format!("claims/{id}/index.json"))).unwrap())
            .unwrap();
    assert_eq!(registry["payload"]["policy_refs"], json!([]));
    assert!(registry["payload"].get("access").is_none());
    assert!(registry["payload"].get("policies").is_none());
    assert_eq!(
        store.registry(id).unwrap().registry.policy_refs.ids().len(),
        0
    );
}

#[test]
fn ensure_rejects_embedded_policy_authority_before_claim_work() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let store = Store::create(&root).unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let path = temp.path().join("ensure.json");
    let baseline = fs::read(root.join("root.json")).unwrap();
    for field in ["overrides", "adopted", "policies"] {
        let mut input = json!({"scope":"main", "source_uuid":Uuid::nil(), "selector":{"type":"local"}, "policy_refs":[]});
        input[field] = json!({});
        assert!(
            serde_json::from_value::<saucepan::models::claim::EnsureRequest>(input.clone())
                .is_err()
        );
        fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
        error(
            run(
                &root,
                &[
                    "ensure-claim",
                    &app.uuid.to_string(),
                    path.to_str().unwrap(),
                ],
            ),
            "invalid_request",
        );
    }
    assert_eq!(baseline, fs::read(root.join("root.json")).unwrap());
    assert!(store.claims(app.uuid).unwrap().is_empty());
}
