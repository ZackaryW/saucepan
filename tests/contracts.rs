use saucepan::{
    core::Store,
    models::{source::Source, store::Request},
};

#[test]
fn persistence_and_legacy_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let store = Store::create(&root).unwrap();
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
    assert!(Store::create(&legacy).is_err());
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
    saucepan::core::policy::validate(&request).unwrap();
    let mut bad = value;
    bad["policies"] = serde_json::json!({"identical":"surprise"});
    assert!(serde_json::from_value::<Request>(bad).is_err());
    let root = tempfile::tempdir().unwrap();
    let store = Store::create(root.path()).unwrap();
    let app = store.register_app("app".into()).unwrap();
    let path = root.path().join(format!("index/{}/index.json", app.uuid));
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["uuid"] = serde_json::json!(uuid::Uuid::new_v4());
    std::fs::write(path, value.to_string()).unwrap();
    assert!(store.record(app.uuid).is_err());
}
