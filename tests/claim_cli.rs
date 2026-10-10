mod native_root;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(root)
        .args(["--error-format", "json"])
        .args(args)
        .output()
        .unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let result = cli(root, args);
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    serde_json::from_slice(&result.stdout).unwrap()
}
fn fail(root: &Path, args: &[&str], code: &str) {
    let result = cli(root, args);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let error: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(error["error"]["code"], code, "{error}");
    assert!(error["error"]["details"].is_object());
}
fn write(path: &Path, value: Value) {
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn credential_files_registration_ensure_recovery_rotation_and_json_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    ok(&root, &["init"]);
    let _cleanup = native_root::open(&root).unwrap();
    let file = tmp.path().join("input");
    fs::write(&file, "bytes").unwrap();
    let source_file = tmp.path().join("source.json");
    write(&source_file, json!({"type":"local", "path":file}));
    let source = ok(&root, &["register-source", source_file.to_str().unwrap()]);
    let registration = tmp.path().join("registration.json");
    write(
        &registration,
        json!({"registration_key":"integration", "name":"consumer", "credentials_required":true,
        "access":{"sources":[source["uuid"]], "scopes":["project-a", "project-b"]}}),
    );
    let secret = tmp.path().join("credential");
    let before = fs::read(root.join("root.json")).unwrap();
    fail(
        &root,
        &["register-application", registration.to_str().unwrap()],
        "invalid_request",
    );
    assert_eq!(fs::read(root.join("root.json")).unwrap(), before);
    let bad_destination = tmp.path().join("missing/secret");
    fail(
        &root,
        &[
            "register-application",
            registration.to_str().unwrap(),
            "--credential-out",
            bad_destination.to_str().unwrap(),
        ],
        "io_error",
    );
    assert_eq!(fs::read(root.join("root.json")).unwrap(), before);
    let app = ok(
        &root,
        &[
            "register-application",
            registration.to_str().unwrap(),
            "--credential-out",
            secret.to_str().unwrap(),
        ],
    );
    let id = app["uuid"].as_str().unwrap();
    assert_eq!(fs::read(&secret).unwrap().len(), 32);
    let repeated = ok(
        &root,
        &[
            "--credential-file",
            secret.to_str().unwrap(),
            "register-application",
            registration.to_str().unwrap(),
        ],
    );
    assert_eq!(repeated["uuid"], id);
    fail(&root, &["claims", id], "authentication_required");
    fail(
        &root,
        &["update-claim", id, &uuid::Uuid::new_v4().to_string()],
        "authentication_required",
    );
    let ensure_file = tmp.path().join("ensure.json");
    write(
        &ensure_file,
        json!({"scope":"project-a", "source_uuid":source["uuid"], "selector":{"type":"local"},
        "context":{"project":"a"}, "overrides":{"keep":1,"reclaim":{"type":"reclaim_policy","strategy":"restore_exact"}}}),
    );
    let first = ok(
        &root,
        &[
            "--credential-file",
            secret.to_str().unwrap(),
            "ensure-claim",
            id,
            ensure_file.to_str().unwrap(),
        ],
    );
    let retry = ok(
        &root,
        &[
            "--credential-file",
            secret.to_str().unwrap(),
            "ensure-claim",
            id,
            ensure_file.to_str().unwrap(),
        ],
    );
    assert_eq!(first["claim"]["uuid"], retry["claim"]["uuid"]);
    let uuid = first["claim"]["entry_uuid"].as_str().unwrap();
    ok(
        &root,
        &[
            "--credential-file",
            secret.to_str().unwrap(),
            "--as-app",
            id,
            "path",
            uuid,
        ],
    );
    let query = tmp.path().join("query.json");
    write(&query, json!({"app":id,"where":[]}));
    assert_eq!(
        ok(
            &root,
            &[
                "--credential-file",
                secret.to_str().unwrap(),
                "query-claims",
                query.to_str().unwrap()
            ]
        )["claims"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    write(&query, json!({"where":[]}));
    assert_eq!(
        ok(&root, &["discover-claims", query.to_str().unwrap()]),
        json!([])
    );
    let config = tmp.path().join("config.json");
    let access = json!({"sources":[source["uuid"]],"scopes":["project-a","project-b"]});
    write(
        &config,
        json!({"expected_revision":1,"access":access,"policies":{},"credential":"rotate"}),
    );
    fail(
        &root,
        &[
            "--credential-file",
            secret.to_str().unwrap(),
            "configure-application",
            id,
            config.to_str().unwrap(),
            "--credential-out",
            secret.to_str().unwrap(),
        ],
        "io_error",
    );
    let rotated = tmp.path().join("rotated");
    let configured = ok(
        &root,
        &[
            "--credential-file",
            secret.to_str().unwrap(),
            "configure-application",
            id,
            config.to_str().unwrap(),
            "--credential-out",
            rotated.to_str().unwrap(),
        ],
    );
    assert_eq!(configured["revision"], 2);
    fail(
        &root,
        &["--credential-file", secret.to_str().unwrap(), "claims", id],
        "authentication_failed",
    );
    fail(
        &root,
        &[
            "--credential-file",
            rotated.to_str().unwrap(),
            "configure-application",
            id,
            config.to_str().unwrap(),
        ],
        "revision_conflict",
    );
    let claimed = first["claim"]["uuid"].as_str().unwrap();
    ok(
        &root,
        &[
            "--credential-file",
            rotated.to_str().unwrap(),
            "recover-claim",
            id,
            claimed,
        ],
    );
    let child = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(&root)
        .args(["--error-format", "json", "--credential-file"])
        .arg(&rotated)
        .args(["run-claim", id, claimed, "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "child_streams", "--nocapture"])
        .env("SAUCEPAN_CLAIM_CHILD", "yes")
        .output()
        .unwrap();
    assert_eq!(child.status.code(), Some(9));
    assert!(String::from_utf8_lossy(&child.stdout).contains("child-output"));
    assert_eq!(child.stderr, b"child-error\n");
    fail(&root, &["--as-app", id, "inspect", id], "access_denied");
    fail(&root, &["--bogus"], "invalid_request");
    fail(&root, &["path", "malformed"], "invalid_request");
    for flag in ["--help", "--version"] {
        assert!(cli(&root, &[flag]).status.success());
    }
    let missing_root = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .env("USERPROFILE", tmp.path().join("isolated-home"))
        .env("HOME", tmp.path().join("isolated-home"))
        .args(["--error-format=json", "claims", id])
        .output()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&missing_root.stderr).unwrap()["error"]["code"],
        "unsupported_store"
    );
}

#[test]
fn child_streams() {
    if std::env::var_os("SAUCEPAN_CLAIM_CHILD").is_none() {
        return;
    }
    let directory = std::env::var_os("SAUCEPAN_RESOURCE_DIR").unwrap();
    assert_eq!(
        fs::read(Path::new(&directory).join("input")).unwrap(),
        b"bytes"
    );
    println!("child-output");
    eprintln!("child-error");
    std::process::exit(9);
}
