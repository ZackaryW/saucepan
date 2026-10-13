mod native_root;
use saucepan::{
    core::Store,
    models::{source::Source, store::Request},
};

#[test]
fn persistence_and_legacy_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = native_root::create(&root).unwrap();
    let app = store.register_app("example".into()).unwrap();
    assert_eq!(
        Store::open(&root)
            .unwrap()
            .application(app.uuid)
            .unwrap()
            .name,
        "example"
    );
    let source = store
        .register_source(Source::Local {
            path: dir.path().to_path_buf(),
        })
        .unwrap();
    assert_eq!(store.source(source.uuid).unwrap().source, source.source);
    assert!(!root.join("store").join(app.uuid.to_string()).exists());
    assert!(Store::open(&dir.path().join("absent")).is_err());
    let legacy = dir.path().join("legacy");
    std::fs::create_dir(&legacy).unwrap();
    std::fs::write(legacy.join("index.json.enc"), b"old").unwrap();
    assert!(native_root::create(&legacy).is_err());
    assert!(Store::open(&legacy).is_err());
    assert_eq!(
        std::fs::read(legacy.join("index.json.enc")).unwrap(),
        b"old"
    );
}
#[test]
fn strict_request_and_index_identity() {
    let value =
        serde_json::json!({"source_uuid":uuid::Uuid::new_v4(), "selector":{"type":"local"}});
    let request: Request = serde_json::from_value(value.clone()).unwrap();
    assert!(request.policy_refs.ids().is_empty());
    let mut bad = value;
    bad["policies"] = serde_json::json!({"identical":"surprise"});
    assert!(serde_json::from_value::<Request>(bad).is_err());
    let root = tempfile::tempdir().unwrap();
    let store = native_root::create(root.path()).unwrap();
    let app = store.register_app("app".into()).unwrap();
    let path = root.path().join(format!("index/{}/index.json", app.uuid));
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["uuid"] = serde_json::json!(uuid::Uuid::new_v4());
    std::fs::write(path, value.to_string()).unwrap();
    assert!(store.record(app.uuid).is_err());
}

#[test]
fn managed_inputs_reject_nested_policy_and_execution_evidence() {
    use saucepan::models::index::lifecycle::Installation;
    let input = serde_json::json!({
        "source_uuid": uuid::Uuid::new_v4(), "selector": {"type":"local"},
        "policy_refs": [], "selection": {"filter_refs": []}
    });
    for (field, value) in [
        ("policies", serde_json::json!({"identical":"reuse"})),
        ("adopted", serde_json::Value::Null),
        ("overrides", serde_json::json!({})),
        (
            "eviction",
            serde_json::json!({"strategy":"latest","keep":10}),
        ),
    ] {
        let mut mixed = input.clone();
        mixed[field] = value;
        assert!(
            serde_json::from_value::<Request>(mixed.clone()).is_err(),
            "{field}"
        );
        let install = serde_json::json!({"scope":"app", "name":"example", "request":mixed,
            "probe":{"args":[],"stdout_contains":"ok","timeout_seconds":1},
            "shim":"example", "saucepan":false});
        assert!(
            serde_json::from_value::<Installation>(install).is_err(),
            "{field}"
        );
    }
    for field in ["filters", "selection_policy"] {
        let mut mixed = input.clone();
        mixed["selection"][field] = serde_json::json!([]);
        assert!(serde_json::from_value::<Request>(mixed).is_err(), "{field}");
    }
}
