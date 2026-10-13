mod native_root;
use saucepan::models::{
    index::Settings,
    source::Source,
    store::{Request, Selector},
};
use std::fs;
#[test]
fn claims_remain_independent_and_failure_preserves_associations() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::create_dir(&input).unwrap();
    for folder in ["a", "b"] {
        fs::create_dir(input.join(folder)).unwrap();
        fs::write(input.join(folder).join("file"), b"one").unwrap();
    }
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local {
            path: input.clone(),
        })
        .unwrap();
    let app_a = store.register_app("A".into()).unwrap();
    let app_b = store.register_app("B".into()).unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policy_refs: Default::default(),
    };
    request.selection.folder = Some("a".into());
    let a = store
        .acquire_for(
            app_a.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    let b = store
        .acquire_for(
            app_b.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(a.acquired.entry.uuid, b.acquired.entry.uuid);
    request.selection.folder = Some("b".into());
    let extra = store
        .acquire_for(
            app_a.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    assert_ne!(extra.claim.uuid, a.claim.uuid);
    fs::write(input.join("a/file"), b"two").unwrap();
    let updated = store
        .update_claim(app_b.uuid, b.claim.uuid, &Default::default())
        .unwrap();
    assert_ne!(updated.claim.entry_uuid, a.claim.entry_uuid);
    assert_eq!(
        store.registry(app_a.uuid).unwrap().claims[&a.claim.uuid].entry_uuid,
        a.claim.entry_uuid
    );
    fs::remove_file(input.join("a/file")).unwrap();
    fs::remove_dir(input.join("a")).unwrap();
    assert!(
        store
            .update_claim(app_b.uuid, b.claim.uuid, &Default::default())
            .is_err()
    );
    assert_eq!(
        store.registry(app_b.uuid).unwrap().claims[&b.claim.uuid].entry_uuid,
        updated.claim.entry_uuid
    );
    store.release_claim(app_a.uuid, a.claim.uuid).unwrap();
    assert_eq!(store.claims(app_b.uuid).unwrap().len(), 1);
    let other = store
        .register_source(Source::Local {
            path: input.join("b"),
        })
        .unwrap();
    store
        .set_settings(
            app_a.uuid,
            Settings {
                source_filter: vec![other.uuid],
            },
        )
        .unwrap();
    assert!(store.claims(app_a.uuid).unwrap().is_empty());
    assert_eq!(store.registry(app_a.uuid).unwrap().claims.len(), 1);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .args(["claims", &app_b.uuid.to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let claims: Vec<saucepan::models::index::ClaimView> =
        serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(claims[0].claim.entry_uuid, updated.claim.entry_uuid);
}
