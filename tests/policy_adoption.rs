use saucepan::{
    core::{Store, policy::fence::PolicyFence},
    error::{Error, ErrorCode},
    models::{
        claim::EnsureRequest,
        policy::{PolicyRecord, PolicyRefs},
        source::Source,
        store::Selector,
    },
};
use serde_json::json;
use uuid::Uuid;

fn record(mut value: serde_json::Value) -> PolicyRecord {
    value["uuid"] = json!(Uuid::new_v4());
    serde_json::from_value(value).unwrap()
}
fn refs(records: &[&PolicyRecord]) -> PolicyRefs {
    records
        .iter()
        .map(|r| r.uuid)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}

#[test]
fn source_adoption_is_revision_checked_and_unrelated_registration_does_not_break_fence() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    std::fs::write(&input, "content").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let policy = record(json!({"type":"source_visibility_policy","public_discovery":true}));
    let fence = PolicyFence::capture(&store, Some(source.uuid), None).unwrap();
    store
        .register_policies(std::slice::from_ref(&policy))
        .unwrap();
    fence.check(&store).unwrap();
    let adopted = store
        .adopt_source_policies(source.uuid, 0, refs(&[&policy]))
        .unwrap();
    assert!(adopted.public_discovery);
    assert_eq!(adopted.source, source.source);
    assert_eq!(adopted.discovery.len(), source.discovery.len());
    assert_eq!(adopted.policy_revision, 1);
    assert_eq!(
        code(fence.check(&store).unwrap_err()),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        code(
            store
                .adopt_source_policies(source.uuid, 0, refs(&[&policy]))
                .unwrap_err()
        ),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        store
            .adopt_source_policies(source.uuid, 1, refs(&[&policy]))
            .unwrap()
            .policy_revision,
        1
    );
}

#[test]
fn app_defaults_do_not_move_claims_and_explicit_adoption_preserves_context_and_output() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    std::fs::write(&input, "content").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let request = EnsureRequest {
        adopted: None,
        scope: "main".to_string().try_into().unwrap(),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: [("label".into(), "kept".into())].into(),
        overrides: Default::default(),
    };
    let original = store
        .ensure_claim(app.uuid, &request, &Default::default())
        .unwrap()
        .claim;
    let five = record(json!({"type":"revision_policy","history":"latest","keep":5}));
    let three = record(json!({"type":"revision_policy","history":"latest","keep":3}));
    let bounds = record(json!({"type":"policy_constraints","maximum_keep":3}));
    store
        .register_policies(&[five.clone(), three.clone(), bounds.clone()])
        .unwrap();
    let revision = store.application(app.uuid).unwrap().revision;
    store
        .adopt_application_policies(app.uuid, revision, refs(&[&five]), None)
        .unwrap();
    let current = store.registry(app.uuid).unwrap().claims[&original.uuid].clone();
    let session = store.session(app.uuid, None).unwrap();
    assert_eq!(session.policy(five.uuid).unwrap(), five);
    assert_eq!(
        code(session.policy(three.uuid).unwrap_err()),
        ErrorCode::AccessDenied
    );
    assert_eq!(current.effective, original.effective);
    assert_eq!(current.context, original.context);
    assert_eq!(current.entry_uuid, original.entry_uuid);
    assert_eq!(
        code(
            store
                .adopt_application_policies(app.uuid, revision, refs(&[&five]), None)
                .unwrap_err()
        ),
        ErrorCode::RevisionConflict
    );
    let revision = store.application(app.uuid).unwrap().revision;
    assert_eq!(
        code(
            store
                .adopt_application_policies(app.uuid, revision, refs(&[&three, &bounds]), None)
                .unwrap_err()
        ),
        ErrorCode::PolicyConflict
    );
    assert_eq!(store.application(app.uuid).unwrap().revision, revision);
    assert_eq!(
        code(
            store
                .adopt_source_policies(source.uuid, 0, refs(&[&bounds]))
                .unwrap_err()
        ),
        ErrorCode::PolicyConflict
    );
    assert_eq!(store.source(source.uuid).unwrap().policy_revision, 0);
    store
        .adopt_claim_policies(app.uuid, original.uuid, revision, refs(&[&three]))
        .unwrap();
    let current = store.registry(app.uuid).unwrap().claims[&original.uuid].clone();
    assert_eq!(current.policy_refs, refs(&[&three]));
    assert_eq!(current.context, original.context);
    assert_eq!(current.entry_uuid, original.entry_uuid);
    assert!(store.application(app.uuid).unwrap().active.is_none());
    store
        .adopt_application_policies(app.uuid, revision + 1, refs(&[&three, &bounds]), None)
        .unwrap();
    store.release_claim(app.uuid, original.uuid).unwrap();
    store.reclaim().unwrap();
    assert_eq!(store.policy(three.uuid).unwrap(), three);
    store.uninstall(app.uuid, false, None).unwrap();
    assert_eq!(store.policy(three.uuid).unwrap(), three);
}

#[test]
fn source_conveniences_publish_immutable_records_and_references_together() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let store = Store::create(&root).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/repo".into(),
        })
        .unwrap();
    let initial: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("root.json")).unwrap()).unwrap();
    let public = store.set_source_visibility(source.uuid, true).unwrap();
    assert_eq!(public.policy_revision, 1);
    assert_eq!(public.policy_refs.ids().len(), 1);
    let old = *public.policy_refs.ids().first().unwrap();
    let definition = store.policy(old).unwrap();
    let after = std::fs::read(root.join("root.json")).unwrap();
    let after_value: serde_json::Value = serde_json::from_slice(&after).unwrap();
    assert_eq!(
        after_value["payload"]["generation"].as_u64().unwrap(),
        initial["payload"]["generation"].as_u64().unwrap() + 1
    );
    store.set_source_visibility(source.uuid, true).unwrap();
    assert_eq!(std::fs::read(root.join("root.json")).unwrap(), after);
    let private = store.set_source_visibility(source.uuid, false).unwrap();
    assert_eq!(private.policy_revision, 2);
    assert_ne!(private.policy_refs, public.policy_refs);
    assert_eq!(store.policy(old).unwrap(), definition);
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("source/{}.json", source.uuid))).unwrap(),
    )
    .unwrap();
    for field in ["public_discovery", "refresh", "retrieval_freshness"] {
        assert!(record["payload"].get(field).is_none());
        let mut mixed = record["payload"].clone();
        mixed[field] = serde_json::json!(null);
        assert!(serde_json::from_value::<saucepan::models::source::SourceRecord>(mixed).is_err());
    }
    assert!(
        !Store::open(&root)
            .unwrap()
            .source(source.uuid)
            .unwrap()
            .public_discovery
    );
}
