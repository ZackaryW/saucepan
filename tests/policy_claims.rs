use saucepan::{
    core::Store,
    error::{Error, ErrorCode},
    models::{
        claim::request::Ensure,
        policy::{PolicyRecord, PolicyRefs, RevisionPolicy},
        source::Source,
        store::Selector,
    },
};
use serde_json::json;
use uuid::Uuid;
fn policy(mut value: serde_json::Value) -> PolicyRecord {
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

#[test]
fn equivalent_definitions_reuse_scoped_claims_and_defaults_do_not_adopt_for_survivors() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    std::fs::create_dir_all(input.join("nested")).unwrap();
    std::fs::write(input.join("nested/file"), "content").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let access = policy(json!({"type":"access_policy","scopes":["main","other"]}));
    let three = policy(json!({"type":"revision_policy","history":"latest","keep":3}));
    let equivalent = policy(json!({"type":"revision_policy","history":"latest","keep":3}));
    let five = policy(json!({"type":"revision_policy","history":"latest","keep":5}));
    store
        .register_policies(&[
            access.clone(),
            three.clone(),
            equivalent.clone(),
            five.clone(),
        ])
        .unwrap();
    store
        .adopt_application_policies(app.uuid, app.revision, refs(&[&access, &three]), None)
        .unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let mut request = Ensure {
        scope: "main".to_string().try_into().unwrap(),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: [("label".into(), "stable".into())].into(),
        policy_refs: Default::default(),
    };
    let first = session
        .ensure_referenced(&request, &Default::default())
        .unwrap();
    assert!(first.claim.policy_refs.ids().contains(&three.uuid));
    assert_eq!(
        first
            .claim
            .request
            .adopted
            .as_ref()
            .unwrap()
            .behavior
            .revision,
        RevisionPolicy::Latest { keep: 3 }
    );
    request.selection.folder = Some("nested".into());
    request.policy_refs = refs(&[&equivalent]);
    let covered = session
        .ensure_referenced(&request, &Default::default())
        .unwrap();
    assert_eq!(covered.claim.uuid, first.claim.uuid);
    assert_eq!(covered.claim.policy_refs, first.claim.policy_refs);
    assert_eq!(covered.view.relative.as_deref(), Some("nested"));
    assert_eq!(session.policy(three.uuid).unwrap(), three);
    assert!(session.policy(equivalent.uuid).is_err());
    let revision = store.application(app.uuid).unwrap().revision;
    store
        .adopt_application_policies(app.uuid, revision, refs(&[&access, &five]), None)
        .unwrap();
    request.policy_refs = Default::default();
    request.selection.folder = None;
    let still_three = session
        .ensure_referenced(&request, &Default::default())
        .unwrap();
    assert_eq!(still_three.claim.policy_refs, first.claim.policy_refs);
    request.scope = "other".to_string().try_into().unwrap();
    let other = session
        .ensure_referenced(&request, &Default::default())
        .unwrap();
    assert_ne!(other.claim.uuid, first.claim.uuid);
    assert!(other.claim.policy_refs.ids().contains(&five.uuid));
    let revision = store.application(app.uuid).unwrap().revision;
    store
        .adopt_claim_policies(app.uuid, first.claim.uuid, revision, refs(&[&five]))
        .unwrap();
    let current = store.registry(app.uuid).unwrap();
    assert_eq!(current.claims.len(), 2);
    assert_eq!(
        current.claims[&first.claim.uuid].context,
        first.claim.context
    );
    assert_eq!(
        current.claims[&first.claim.uuid].entry_uuid,
        first.claim.entry_uuid
    );
    assert_eq!(
        current.claims[&other.claim.uuid].policy_refs,
        other.claim.policy_refs
    );
    // A current source restriction also rejects an existing cached claim.
    let deny_read = policy(json!({"type":"access_policy","actions":["acquire"]}));
    store
        .register_policies(std::slice::from_ref(&deny_read))
        .unwrap();
    store
        .adopt_source_policies(source.uuid, 0, refs(&[&deny_read]))
        .unwrap();
    let error = session.policy(five.uuid);
    // App association is independently readable only if its own dimensions allow it.
    assert_eq!(
        Error::from_anyhow(&error.unwrap_err()).code,
        ErrorCode::AccessDenied
    );
}

#[test]
fn automatic_claims_retain_adopted_evidence_through_publication_and_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    std::fs::write(&input, "one").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local {
            path: input.clone(),
        })
        .unwrap();
    let app = store.register_app("updates".into()).unwrap();
    let update = policy(json!({"type":"update_policy","mode":"always"}));
    let no_history = policy(json!({"type":"revision_policy","history":"none"}));
    store
        .register_policies(&[update.clone(), no_history.clone()])
        .unwrap();
    let request = Ensure {
        scope: "main".to_string().try_into().unwrap(),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: Default::default(),
        policy_refs: refs(&[&update, &no_history]),
    };
    let first = store
        .ensure_referenced(app.uuid, &request, &Default::default())
        .unwrap();
    assert_eq!(
        first.claim.effective.output.revision,
        RevisionPolicy::None {}
    );
    assert!(first.claim.policy_refs.ids().contains(&update.uuid));
    assert!(first.claim.policy_refs.ids().contains(&no_history.uuid));
    std::fs::write(&input, "two").unwrap();
    let second = store
        .ensure_referenced(app.uuid, &request, &Default::default())
        .unwrap();
    assert_eq!(second.claim.uuid, first.claim.uuid);
    assert_ne!(second.claim.entry_uuid, first.claim.entry_uuid);
    let reopened = Store::open(store.root()).unwrap();
    let registry = reopened.registry(app.uuid).unwrap();
    assert_eq!(
        registry.claims[&first.claim.uuid].request.policies.revision,
        RevisionPolicy::None {}
    );
    assert_eq!(
        registry.claims[&first.claim.uuid].request.adopted,
        first.claim.request.adopted
    );
}

#[test]
fn protection_defaults_do_not_reinterpret_surviving_claims() {
    for automatic in [false, true] {
        for configure in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let input = temp.path().join("input");
            std::fs::write(&input, "one").unwrap();
            let store = Store::create(&temp.path().join("root")).unwrap();
            let source = store
                .register_source(Source::Local {
                    path: input.clone(),
                })
                .unwrap();
            let app = store.register_app("consumer".into()).unwrap();
            let protect = policy(json!({"type":"useable_app_policy"}));
            let update = policy(json!({"type":"update_policy","mode":"always"}));
            let required = policy(json!({"type":"policy_constraints","require_useable_app":true}));
            store
                .register_policies(&[protect.clone(), update.clone(), required.clone()])
                .unwrap();
            let request = Ensure {
                scope: "main".to_string().try_into().unwrap(),
                source_uuid: source.uuid,
                selector: Selector::Local,
                selection: Default::default(),
                context: Default::default(),
                policy_refs: if automatic {
                    refs(&[&update])
                } else {
                    Default::default()
                },
            };
            let first = store
                .ensure_claim(app.uuid, &request, &Default::default())
                .unwrap();
            let revision = store.application(app.uuid).unwrap().revision;
            if configure {
                store
                    .configure_application(
                        app.uuid,
                        saucepan::core::claim::session::ApplicationConfiguration {
                            expected_revision: revision,
                            policy_refs: refs(&[&protect]),
                            credential: None,
                        },
                        None,
                    )
                    .unwrap();
            } else {
                store
                    .adopt_application_policies(app.uuid, revision, refs(&[&protect]), None)
                    .unwrap();
            }
            std::fs::write(&input, "two").unwrap();
            let next = store
                .ensure_claim(app.uuid, &request, &Default::default())
                .unwrap();
            assert_eq!(first.claim.uuid, next.claim.uuid);
            assert_eq!(first.claim.request.adopted, next.claim.request.adopted);
            assert_eq!(next.claim.effective.output.protection, None);
            let reopened = Store::open(store.root()).unwrap();
            let revision = reopened.application(app.uuid).unwrap().revision;
            // A default may change, but an actual constraint cannot silently
            // turn an existing unprotected claim into a protected one.
            let error = reopened
                .adopt_application_policies(app.uuid, revision, refs(&[&required]), None)
                .unwrap_err();
            assert_eq!(Error::from_anyhow(&error).code, ErrorCode::PolicyConflict);
            assert_eq!(reopened.application(app.uuid).unwrap().revision, revision);
            assert_eq!(
                reopened.registry(app.uuid).unwrap().claims[&first.claim.uuid]
                    .effective
                    .output
                    .protection,
                None
            );
        }
    }
}
