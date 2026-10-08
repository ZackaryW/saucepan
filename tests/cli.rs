use std::process::Command;

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
            .application(uuid::Uuid::parse_str(app["uuid"].as_str().unwrap()).unwrap())
            .unwrap()
            .claims
            .len(),
        0
    );
    assert!(!temp.path().join(".saucepan").exists());
}
