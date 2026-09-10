use saucepan::core::{Store, models::*};

#[test]
fn custom_root_and_key_are_explicit_persistent_and_fail_without_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("custom-store");
    assert!(Store::open_test(&root, [37; 32]).is_err());
    assert!(!root.exists());
    let store = Store::create_test(&root, [37; 32]).unwrap();
    let app = AppContext::authenticated(
        store
            .register("app", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    let before = std::fs::read(root.join("index.json.enc")).unwrap();
    let reopened = Store::open_test(&root, [37; 32]).unwrap();
    reopened
        .verify_view(&app, &store.view(&app).unwrap())
        .unwrap();
    assert!(!root.join(".saucepan").exists());
    assert!(!root.join("test-store.json").exists());
    assert!(Store::open_test(&root, [38; 32]).is_err());
    assert!(Store::create_test(&root, [38; 32]).is_err());
    assert_eq!(std::fs::read(root.join("index.json.enc")).unwrap(), before);
    std::fs::remove_file(root.join("index.json.enc")).unwrap();
    assert!(Store::open_test(&root, [37; 32]).is_err());
    assert!(!root.join("index.json.enc").exists());
}

#[test]
fn store_ciphertexts_and_caller_tokens_are_not_interchangeable() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    let first = Store::create_test(&a, [1; 32]).unwrap();
    let second = Store::create_test(&b, [2; 32]).unwrap();
    let app = AppContext::authenticated(
        first
            .register("app", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    second
        .register("app", AppSettings::default(), Filters::default())
        .unwrap();
    assert!(second.view(&app).is_err());
    std::fs::copy(a.join("index.json.enc"), b.join("index.json.enc")).unwrap();
    assert!(Store::open_test(&b, [2; 32]).is_err());
    Store::open_test(&a, [1; 32]).unwrap().view(&app).unwrap();
}
