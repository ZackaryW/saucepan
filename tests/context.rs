use saucepan::{
    core::Store,
    models::{
        index::{Claim, ClaimContext, validate_context},
        policy::{Eviction, Identical},
        source::Source,
        store::{Availability, Request, Selector},
    },
};
use std::fs;

fn context(project: &str) -> ClaimContext {
    [
        ("project".into(), project.into()),
        ("source.alias".into(), "skills-*".into()),
    ]
    .into()
}

#[test]
fn context_validation_and_plain_serialization() {
    let valid = context(r"C:\work\项目");
    assert_eq!(
        serde_json::from_value::<ClaimContext>(serde_json::to_value(&valid).unwrap()).unwrap(),
        valid
    );
    for bad in [
        serde_json::json!({"x": 1}),
        serde_json::json!({"x": {"glob": "*"}}),
        serde_json::json!({"x": null}),
    ] {
        assert!(serde_json::from_value::<ClaimContext>(bad).is_err());
    }
    for bad in [
        [("".into(), "x".into())].into(),
        [("é".repeat(65), "x".into())].into(),
        [("x".into(), "é".repeat(2049))].into(),
        (0..65).map(|i| (i.to_string(), String::new())).collect(),
    ] {
        assert!(validate_context(&bad).is_err());
    }
    validate_context(&[("k".repeat(128), "v".repeat(4096))].into()).unwrap();
    validate_context(&[(" ".into(), String::new())].into()).unwrap();
}

#[test]
fn context_publication_replacement_reclaim_and_release() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, "one").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let source = store
        .register_source(Source::Local { path: file.clone() })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.policies.eviction = Eviction::Latest { keep: 1 };
    let a = store
        .acquire_for_with_context(app.uuid, &request, &context("A"), &Default::default())
        .unwrap();
    let b = store
        .acquire_for_with_context(app.uuid, &request, &context("B"), &Default::default())
        .unwrap();
    assert_eq!(a.acquired.entry.uuid, b.acquired.entry.uuid);
    assert_ne!(a.claim.uuid, b.claim.uuid);
    assert_eq!(a.claim.context, context("A"));
    let mut json = serde_json::to_value(&a.claim).unwrap();
    assert_eq!(
        serde_json::from_value::<Claim>(json.clone())
            .unwrap()
            .context,
        context("A")
    );
    json.as_object_mut().unwrap().remove("context");
    assert!(
        serde_json::from_value::<Claim>(json)
            .unwrap()
            .context
            .is_empty()
    );
    let before = store.application(app.uuid).unwrap();
    let revision = before.revision;
    let changed = store
        .set_claim_context(app.uuid, a.claim.uuid, &context("C"), revision)
        .unwrap();
    assert_eq!(changed.entry_uuid, a.claim.entry_uuid);
    assert_eq!(
        serde_json::to_value(&changed.request).unwrap(),
        serde_json::to_value(&a.claim.request).unwrap()
    );
    let after = store.application(app.uuid).unwrap();
    assert_eq!(after.revision, revision + 1);
    assert_eq!(after.claims[&b.claim.uuid].context, context("B"));
    assert_eq!(
        serde_json::to_value(&after.active).unwrap(),
        serde_json::to_value(&before.active).unwrap()
    );
    assert!(
        store
            .set_claim_context(app.uuid, a.claim.uuid, &context("C"), revision)
            .is_err()
    );
    store
        .set_claim_context(app.uuid, a.claim.uuid, &context("C"), revision + 1)
        .unwrap();
    assert_eq!(store.application(app.uuid).unwrap().revision, revision + 1);
    let cleared = store
        .set_claim_context(app.uuid, a.claim.uuid, &Default::default(), revision + 1)
        .unwrap();
    assert!(cleared.context.is_empty());
    assert!(
        store
            .set_claim_context(app.uuid, uuid::Uuid::new_v4(), &context("X"), revision + 2)
            .is_err()
    );
    assert!(
        store
            .set_claim_context(uuid::Uuid::new_v4(), a.claim.uuid, &context("X"), 0)
            .is_err()
    );
    fs::write(&file, "two").unwrap();
    let updated = store
        .update_claim(app.uuid, b.claim.uuid, &Default::default())
        .unwrap();
    assert_eq!(updated.claim.context, context("B"));
    assert_eq!(
        store.output(a.claim.entry_uuid).unwrap().state,
        Availability::Reclaimed
    );
    assert!(
        store.application(app.uuid).unwrap().claims[&a.claim.uuid]
            .context
            .is_empty()
    );
    let before_failure = serde_json::to_value(store.application(app.uuid).unwrap()).unwrap();
    fs::remove_file(&file).unwrap();
    assert!(
        store
            .acquire_for_with_context(app.uuid, &request, &context("D"), &Default::default())
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(store.application(app.uuid).unwrap()).unwrap(),
        before_failure
    );
    store.release_claim(app.uuid, b.claim.uuid).unwrap();
    assert!(
        !store
            .application(app.uuid)
            .unwrap()
            .claims
            .contains_key(&b.claim.uuid)
    );
    store.uninstall(app.uuid, false, None).unwrap();
    assert!(
        store
            .set_claim_context(app.uuid, a.claim.uuid, &context("D"), 0)
            .is_err()
    );
    assert!(store.application(app.uuid).is_err());
}

#[test]
fn invalid_context_fails_before_acquiring_or_mutating() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let app = store.register_app("test".into()).unwrap();
    let request = Request {
        source_uuid: uuid::Uuid::new_v4(),
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    let bad = [("".into(), "x".into())].into();
    let err = store
        .acquire_for_with_context(app.uuid, &request, &bad, &Default::default())
        .unwrap_err();
    assert!(err.to_string().contains("context key"));
    assert!(
        store
            .set_claim_context(app.uuid, uuid::Uuid::new_v4(), &bad, 0)
            .unwrap_err()
            .to_string()
            .contains("context key")
    );
    assert_eq!(store.records().unwrap().len(), 1);
    assert_eq!(store.application(app.uuid).unwrap().revision, 0);
    assert_eq!(
        fs::read_dir(store.root().join(".staging")).unwrap().count(),
        0
    );
    assert_eq!(request.policies.identical, Identical::Reuse);
}
