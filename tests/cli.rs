mod native_root;
use std::process::Command;

#[test]
fn default_home_root_is_consistent_and_explicit_override_never_falls_back() {
    use serde_json::{Value, json};
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home with spaces");
    let cwd = tmp.path().join("project");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(cwd.join(".saucepan")).unwrap();
    fs::write(cwd.join(".saucepan/sentinel"), "do not touch").unwrap();
    let root = home.join(".saucepan");
    let call = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .current_dir(&cwd)
            .args(["--error-format", "json"])
            .args(args)
            .output()
            .unwrap()
    };
    let missing = call(&["query-sources"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    let error: Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert_eq!(error["error"]["code"], "unsupported_store");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("saucepan init")
    );
    assert!(!root.exists());
    assert!(call(&["init"]).status.success());
    let _default_cleanup = native_root::open(&root).unwrap();
    let source_file = cwd.join("source.json");
    fs::write(
        &source_file,
        json!({"type":"github_release","repo":"owner/repo"}).to_string(),
    )
    .unwrap();
    let registered = call(&["register-source", "source.json"]);
    assert!(registered.status.success());
    let source: Value = serde_json::from_slice(&registered.stdout).unwrap();
    let id = source["uuid"].as_str().unwrap();
    assert!(
        call(&["set-source-visibility", id, "public"])
            .status
            .success()
    );
    let before = fs::read(root.join("root.json")).unwrap();
    let query = call(&[
        "query-sources",
        "--type",
        "github_release",
        "--repo",
        "owner/repo",
    ]);
    assert!(query.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&query.stdout).unwrap()["sources"][0]["uuid"],
        id
    );
    // Relative override is anchored in the working directory, not the home.
    assert!(call(&["--root", "scratch", "init"]).status.success());
    let _explicit_cleanup = native_root::open(&cwd.join("scratch")).unwrap();
    let explicit = call(&["--root", "scratch", "query-sources"]);
    assert!(explicit.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&explicit.stdout).unwrap()["sources"],
        json!([])
    );
    assert!(
        !call(&["--root", "missing", "query-sources"])
            .status
            .success()
    );
    assert!(!cwd.join("missing").exists());
    let invalid = cwd.join("invalid");
    fs::create_dir(&invalid).unwrap();
    fs::write(invalid.join("root.json"), "{}").unwrap();
    assert!(
        !call(&["--root", "invalid", "query-sources"])
            .status
            .success()
    );
    assert_eq!(fs::read(invalid.join("root.json")).unwrap(), b"{}");
    assert_eq!(fs::read(root.join("root.json")).unwrap(), before);
    assert_eq!(
        fs::read(cwd.join(".saucepan/sentinel")).unwrap(),
        b"do not touch"
    );
    assert!(!cwd.join(".saucepan/root.json").exists());
    // Explicit roots and informational commands do not need a valid home path.
    for args in [
        vec!["--root", "scratch", "query-sources"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .env("HOME", "relative-home")
            .env("USERPROFILE", "relative-home")
            .current_dir(&cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn lifecycle_inspection_reports_pending_without_success_exit() {
    use saucepan::models::index::lifecycle::Action;
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let app = store.register_app("pending".into()).unwrap();
    let id = uuid::Uuid::new_v4();
    let (_, guard) = store
        .begin_operation(
            id,
            Some(app.uuid),
            Action::Uninstall {
                leave_unmanaged_shims: false,
            },
        )
        .unwrap();
    drop(guard);
    let call = |command: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(store.root())
            .args(command)
            .output()
            .unwrap()
    };
    let result = call(&["operations", "--app", &app.uuid.to_string()]);
    assert!(result.status.success());
    let entries: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(entries[0]["operation"], id.to_string());
    let pending = call(&["reconcile", "--operation", &id.to_string()]);
    assert_eq!(pending.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&pending.stdout).unwrap()[0]["reason"]["code"],
        "invalid_request"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&pending.stdout).unwrap()[0]["status"],
        "preparing"
    );
    assert!(store.application(app.uuid).is_ok());
    let missing = call(&[
        "reconcile",
        "--operation",
        &uuid::Uuid::new_v4().to_string(),
    ]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
}

#[test]
fn documented_initialization_and_registration_use_only_explicit_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("scratch");
    let invoke = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    invoke(&["init"]);
    let _native_key_cleanup = native_root::open(&root).unwrap();
    let app = invoke(&["register-app", "example"]);
    assert_eq!(app["name"], "example");
    assert!(
        root.join("index")
            .join(app["uuid"].as_str().unwrap())
            .join("index.json")
            .exists()
    );
    let source = temp.path().join("source.json");
    std::fs::write(&source, r#"{"type":"local","path":"./assets"}"#).unwrap();
    let input = temp.path().join("assets");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("data"), "value").unwrap();
    std::fs::write(
        &source,
        serde_json::json!({"type":"local","path":input}).to_string(),
    )
    .unwrap();
    let registered = invoke(&["register-source", source.to_str().unwrap()]);
    let request_path = temp.path().join("request.json");
    let request = serde_json::json!({"source_uuid":registered["uuid"],"selector":{"type":"local"}});
    std::fs::write(&request_path, request.to_string()).unwrap();
    let acquired = invoke(&["acquire", request_path.to_str().unwrap()]);
    let native = saucepan::core::Store::open(&root)
        .unwrap()
        .acquire(&serde_json::from_value(request).unwrap())
        .unwrap();
    assert_eq!(acquired["entry"]["uuid"], native.entry.uuid.to_string());
    assert_eq!(
        saucepan::core::Store::open(&root)
            .unwrap()
            .registry(uuid::Uuid::parse_str(app["uuid"].as_str().unwrap()).unwrap())
            .unwrap()
            .claims
            .len(),
        0
    );
    assert!(!temp.path().join(".saucepan").exists());
}
