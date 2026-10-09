mod native_root;
use std::process::Command;

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
