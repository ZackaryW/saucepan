mod native_root;
use saucepan::{
    core::{
        Store,
        query::{ClaimQuery, QueryResult},
    },
    models::index::Claimed,
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
        .args(args)
        .output()
        .unwrap()
}
fn success(root: &Path, args: &[&str]) -> Value {
    let out = cli(root, args);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn failure(root: &Path, args: &[&str]) {
    let out = cli(root, args);
    assert!(!out.status.success(), "{args:?}");
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}
fn write(path: &Path, data: &Value) {
    fs::write(path, serde_json::to_vec(data).unwrap()).unwrap();
}

#[test]
fn documented_cli_flow_and_native_parity() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    success(&root, &["init"]);
    let _native_key_cleanup = native_root::open(&root).unwrap();
    let input = temp.path().join("input.txt");
    fs::write(&input, "shared data").unwrap();
    let source = temp.path().join("source.json");
    write(&source, &json!({"type":"local", "path":input}));
    let source = success(&root, &["register-source", source.to_str().unwrap()]);
    let app = success(&root, &["register-app", "consumer"])["uuid"]
        .as_str()
        .unwrap()
        .to_owned();
    let request = temp.path().join("request.json");
    write(
        &request,
        &json!({"source_uuid":source["uuid"], "selector":{"type":"local"}}),
    );
    let context_a = temp.path().join("context-a.json");
    let context_b = temp.path().join("context-b.json");
    fs::write(
        &context_a,
        include_str!("../examples/claim-query/context-a.json"),
    )
    .unwrap();
    fs::write(
        &context_b,
        include_str!("../examples/claim-query/context-b.json"),
    )
    .unwrap();
    let a: Claimed = serde_json::from_value(success(
        &root,
        &[
            "acquire",
            request.to_str().unwrap(),
            "--scope",
            "fixture",
            "--app",
            &app,
            "--context",
            context_a.to_str().unwrap(),
        ],
    ))
    .unwrap();
    let b: Claimed = serde_json::from_value(success(
        &root,
        &[
            "acquire",
            request.to_str().unwrap(),
            "--scope",
            "project-b",
            "--app",
            &app,
            "--context",
            context_b.to_str().unwrap(),
        ],
    ))
    .unwrap();
    assert_eq!(a.claim.entry_uuid, b.claim.entry_uuid);
    assert_ne!(a.claim.uuid, b.claim.uuid);
    let query_path = temp.path().join("query.json");
    let mut query: Value =
        serde_json::from_str(include_str!("../examples/claim-query/query.json")).unwrap();
    query["app"] = app.clone().into();
    write(&query_path, &query);
    let found = success(&root, &["query-claims", query_path.to_str().unwrap()]);
    let native = Store::open(&root)
        .unwrap()
        .query_claims(&serde_json::from_value::<ClaimQuery>(query.clone()).unwrap())
        .unwrap();
    assert_eq!(found, serde_json::to_value(&native).unwrap());
    assert_eq!(native.claims.len(), 1);
    assert_eq!(native.claims[0].claim.uuid, a.claim.uuid);
    let revision = native.revision.to_string();
    let changed = success(
        &root,
        &[
            "set-claim-context",
            &app,
            &a.claim.uuid.to_string(),
            context_b.to_str().unwrap(),
            "--expected-revision",
            &revision,
        ],
    );
    assert_eq!(changed["context"]["project"], "C:/work/project-b");
    assert!(
        success(&root, &["query-claims", query_path.to_str().unwrap()])["claims"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    query["where"][0]["value"] = "C:/work/project-b".into();
    write(&query_path, &query);
    let found: QueryResult = serde_json::from_value(success(
        &root,
        &["query-claims", query_path.to_str().unwrap()],
    ))
    .unwrap();
    assert_eq!(found.claims.len(), 2);
    assert!(found.claims[0].claim.uuid < found.claims[1].claim.uuid);
    assert_eq!(found.revision, native.revision + 1);
    failure(
        &root,
        &[
            "set-claim-context",
            &app,
            &a.claim.uuid.to_string(),
            context_b.to_str().unwrap(),
            "--expected-revision",
            &revision,
        ],
    );
    let before = fs::read(root.join(format!("index/{app}/index.json"))).unwrap();
    failure(
        &root,
        &[
            "acquire",
            request.to_str().unwrap(),
            "--context",
            context_a.to_str().unwrap(),
        ],
    );
    for bad in [
        json!({"x": 3}),
        json!({"x":{"glob":"*"}}),
        json!({"":"invalid"}),
    ] {
        write(&context_a, &bad);
        failure(
            &root,
            &[
                "acquire",
                request.to_str().unwrap(),
                "--scope",
                "fixture",
                "--app",
                &app,
                "--context",
                context_a.to_str().unwrap(),
            ],
        );
    }
    assert_eq!(
        fs::read(root.join(format!("index/{app}/index.json"))).unwrap(),
        before
    );
    for bad in [
        json!({"app":app}),
        json!({"app":app,"where":[{"key":"x","op":"glob","value":"["}]}),
        json!({"app":app,"where":[],"unexpected":true}),
    ] {
        write(&query_path, &bad);
        failure(&root, &["query-claims", query_path.to_str().unwrap()]);
    }
    fs::write(&query_path, "invalid json").unwrap();
    failure(&root, &["query-claims", query_path.to_str().unwrap()]);
    let missing_root = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .args(["query-claims", query_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!missing_root.status.success());
    assert!(missing_root.stdout.is_empty());
    // Ordinary acquisition and clearing context remain explicit supported paths.
    let empty = success(
        &root,
        &[
            "acquire",
            request.to_str().unwrap(),
            "--scope",
            "unlabeled",
            "--app",
            &app,
        ],
    );
    assert_eq!(empty["claim"]["context"], json!({}));
    write(&context_a, &json!({}));
    let revision = Store::open(&root)
        .unwrap()
        .application(app.parse().unwrap())
        .unwrap()
        .revision;
    let cleared = success(
        &root,
        &[
            "set-claim-context",
            &app,
            &a.claim.uuid.to_string(),
            context_a.to_str().unwrap(),
            "--expected-revision",
            &revision.to_string(),
        ],
    );
    assert_eq!(cleared["context"], json!({}));
}
