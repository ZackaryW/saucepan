mod native_root;
use saucepan::{
    core::query::SourceQuery,
    models::{claim::EnsureRequest, store::VerificationMode},
};
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
fn fail(root: &Path, args: &[&str], expected: &str) {
    let result = cli(root, args);
    assert!(!result.status.success(), "{args:?}");
    assert!(result.stdout.is_empty());
    let error: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(error["error"]["code"], expected, "{error}");
}
fn write(path: &Path, value: Value) {
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn simple_app_can_find_unclaimed_source_then_ensure_and_lease() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    ok(&root, &["init"]);
    let store = native_root::open(&root).unwrap();
    let payload = tmp.path().join("payload.txt");
    fs::write(&payload, "consumer bytes").unwrap();
    let input = tmp.path().join("source.json");
    write(&input, json!({"type":"local","path":payload}));
    let source = ok(&root, &["register-source", input.to_str().unwrap()]);
    let source_id = source["uuid"].as_str().unwrap();
    let registration = tmp.path().join("app.json");
    write(
        &registration,
        json!({"registration_key":"simple-app","name":"Simple App"}),
    );
    let app = ok(
        &root,
        &["register-application", registration.to_str().unwrap()],
    );
    let app_id = app["uuid"].as_str().unwrap();
    let file = tmp.path().join("query.json");
    write(&file, json!({}));
    let file = file.to_str().unwrap();
    assert_eq!(ok(&root, &["query-sources", file])["sources"], json!([]));
    let before = fs::read(root.join("root.json")).unwrap();
    let found = ok(&root, &["--as-app", app_id, "query-sources", file]);
    assert_eq!(found["sources"].as_array().unwrap().len(), 1);
    assert_eq!(found["sources"][0]["uuid"], source_id);
    let session = store.session(app_id.parse().unwrap(), None).unwrap();
    assert_eq!(
        found,
        serde_json::to_value(session.query_sources(&SourceQuery::default()).unwrap()).unwrap()
    );
    assert_eq!(fs::read(root.join("root.json")).unwrap(), before);
    assert!(session.claims().unwrap().is_empty());

    let ensure = tmp.path().join("ensure.json");
    let request = json!({"scope":"default","source_uuid":found["sources"][0]["uuid"],"selector":{"type":"local"}});
    write(&ensure, request.clone());
    let claimed = ok(&root, &["ensure-claim", app_id, ensure.to_str().unwrap()]);
    assert_eq!(
        ok(&root, &["ensure-claim", app_id, ensure.to_str().unwrap()])["claim"]["uuid"],
        claimed["claim"]["uuid"]
    );
    let claim_id = claimed["claim"]["uuid"].as_str().unwrap().parse().unwrap();
    let lease = session
        .lease_claim(claim_id, VerificationMode::Full)
        .unwrap();
    assert_eq!(
        fs::read_to_string(lease.directory().join("payload.txt")).unwrap(),
        "consumer bytes"
    );
    let native = session
        .ensure(
            &serde_json::from_value::<EnsureRequest>(request).unwrap(),
            &Default::default(),
        )
        .unwrap();
    assert_eq!(native.claim.uuid, claim_id);
    drop(lease);

    let child = cli(
        &root,
        &[
            "run-claim",
            app_id,
            &claim_id.to_string(),
            "--",
            std::env::current_exe().unwrap().to_str().unwrap(),
            "--exact",
            "source_query_child",
            "--nocapture",
        ],
    );
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(
        String::from_utf8_lossy(&child.stdout).contains("read consumer bytes through run-claim")
    );

    ok(&root, &["set-source-visibility", source_id, "public"]);
    let public = ok(&root, &["query-sources", file]);
    assert_eq!(public["sources"][0]["uuid"], source_id);
    assert_eq!(
        public,
        serde_json::to_value(store.query_sources(&SourceQuery::default()).unwrap()).unwrap()
    );
    assert!(
        ok(&root, &["register-source", input.to_str().unwrap()])["public_discovery"]
            .as_bool()
            .unwrap()
    );
    let discover = tmp.path().join("discover.json");
    write(&discover, json!({"where":[]}));
    assert_eq!(
        ok(&root, &["discover-claims", discover.to_str().unwrap()]),
        json!([])
    );
    fail(
        &root,
        &[
            "--as-app",
            app_id,
            "set-source-visibility",
            source_id,
            "private",
        ],
        "access_denied",
    );
    ok(&root, &["set-source-visibility", source_id, "private"]);
    assert_eq!(ok(&root, &["query-sources", file])["sources"], json!([]));
}

#[test]
fn source_query_child() {
    let Some(directory) = std::env::var_os("SAUCEPAN_RESOURCE_DIR") else {
        return;
    };
    assert_eq!(
        fs::read_to_string(Path::new(&directory).join("payload.txt")).unwrap(),
        "consumer bytes"
    );
    println!("read consumer bytes through run-claim");
}

#[test]
fn cli_enforces_query_shape_credentials_and_source_permissions() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let _store = native_root::create(&root).unwrap();
    let query_file = tmp.path().join("query.json");
    let q = query_file.to_str().unwrap();
    for bad in [
        json!({"scope":"default"}),
        json!({"type":"other"}),
        json!({"repo":""}),
        json!({"uuid":"invalid"}),
    ] {
        write(&query_file, bad);
        fail(&root, &["query-sources", q], "invalid_request");
    }
    write(&query_file, json!({}));
    let registration = tmp.path().join("app.json");
    write(
        &registration,
        json!({"registration_key":"protected","name":"Protected","credentials_required":true}),
    );
    let token = tmp.path().join("credential");
    let app = ok(
        &root,
        &[
            "register-application",
            registration.to_str().unwrap(),
            "--credential-out",
            token.to_str().unwrap(),
        ],
    );
    let id = app["uuid"].as_str().unwrap();
    fail(
        &root,
        &["--as-app", id, "query-sources", q],
        "authentication_required",
    );
    let bad = tmp.path().join("wrong-credential");
    fs::write(&bad, [0u8; 32]).unwrap();
    fail(
        &root,
        &[
            "--as-app",
            id,
            "--credential-file",
            bad.to_str().unwrap(),
            "query-sources",
            q,
        ],
        "authentication_failed",
    );
    assert_eq!(
        ok(
            &root,
            &[
                "--as-app",
                id,
                "--credential-file",
                token.to_str().unwrap(),
                "query-sources",
                q
            ]
        )["sources"],
        json!([])
    );
    let source_file = tmp.path().join("source.json");
    write(
        &source_file,
        json!({"type":"github_release","repo":"owner/repo"}),
    );
    let source = ok(&root, &["register-source", source_file.to_str().unwrap()]);
    let source_id = source["uuid"].as_str().unwrap();
    ok(&root, &["set-source-visibility", source_id, "public"]);
    write(
        &query_file,
        json!({"type":"github_release","repo":"owner/repo"}),
    );
    assert_eq!(
        ok(&root, &["query-sources", q])["sources"][0]["uuid"],
        source_id
    );
    for (key, access, expected_error) in [
        ("denied", json!({"actions":[]}), Some("access_denied")),
        ("none", json!({"sources":[]}), None),
        (
            "discover-only",
            json!({"actions":["discover"],"scopes":[]}),
            None,
        ),
    ] {
        write(
            &registration,
            json!({"registration_key":key,"name":key,"access":access}),
        );
        let app = ok(
            &root,
            &["register-application", registration.to_str().unwrap()],
        );
        let app_id = app["uuid"].as_str().unwrap();
        if let Some(error) = expected_error {
            fail(&root, &["--as-app", app_id, "query-sources", q], error);
        } else {
            let sources = ok(&root, &["--as-app", app_id, "query-sources", q]);
            assert_eq!(
                sources["sources"].as_array().unwrap().len(),
                usize::from(key == "discover-only")
            );
        }
        let ensure = tmp.path().join("ensure.json");
        write(
            &ensure,
            json!({"scope":"default","source_uuid":source_id,"selector":{"type":"latest_stable"}}),
        );
        fail(
            &root,
            &["ensure-claim", app_id, ensure.to_str().unwrap()],
            "access_denied",
        );
    }
    fail(
        &root,
        &["set-source-visibility", source_id, "unknown"],
        "invalid_request",
    );
    fs::write(root.join(format!("source/{source_id}.json")), b"{}").unwrap();
    fail(&root, &["query-sources", q], "integrity_error");
}
