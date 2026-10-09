use super::*;
use crate::models::{
    index::{AppState, Record},
    source::Source,
    store::{Request, Selector},
};
use serde_json::json;
use std::{
    fs,
    sync::{Arc, Barrier},
};
use uuid::Uuid;

#[test]
fn strict_shape_and_validation_even_without_candidates() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let app = store.register_app("empty".into()).unwrap();
    for bad in [
        json!({"app":app.uuid}),
        json!({"where":[]}),
        json!({"app":"bad", "where":[]}),
        json!({"app":app.uuid, "where":[], "extra":true}),
        json!({"app":app.uuid, "where":[{"key":"x","op":"regex","value":"*"}]}),
        json!({"app":app.uuid, "where":[{"key":"x","op":"eq","value":1}]}),
        json!({"app":app.uuid, "where":[{"key":"x","op":"eq","value":"x","extra":true}]}),
        json!({"app":app.uuid, "where":[{"key":"x","value":"x"}]}),
    ] {
        assert!(serde_json::from_value::<ClaimQuery>(bad).is_err());
    }
    for (key, value, op) in [
        ("".into(), "".into(), Operator::Eq),
        ("x".repeat(129), "".into(), Operator::Eq),
        ("x".into(), "é".repeat(2049), Operator::Eq),
        ("x".into(), "[".into(), Operator::Glob),
    ] {
        let query = ClaimQuery {
            app: app.uuid,
            conditions: vec![Condition { key, value, op }],
        };
        assert!(
            store
                .query_claims(&query)
                .unwrap_err()
                .to_string()
                .contains("condition 0")
        );
    }
    let condition = Condition {
        key: "x".into(),
        op: Operator::Eq,
        value: String::new(),
    };
    assert!(
        store
            .query_claims(&ClaimQuery {
                app: app.uuid,
                conditions: vec![condition; 65]
            })
            .is_err()
    );
    let mut query = ClaimQuery {
        app: app.uuid,
        conditions: vec![],
    };
    assert!(store.query_claims(&query).unwrap().claims.is_empty());
    let mut removing = app;
    removing.state = AppState::Removing;
    store.save_record(&Record::Application(removing)).unwrap();
    assert!(store.query_claims(&query).is_err());
    query.app = Uuid::new_v4();
    assert!(store.query_claims(&query).is_err());
}

#[test]
fn coherent_revision_context_and_selection_during_mutation() {
    let tmp = tempfile::tempdir().unwrap();
    let mut store = Store::create(&tmp.path().join("root")).unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, "one").unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let app = store.register_app("test".into()).unwrap();
    let request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    let claim = store
        .acquire_for(app.uuid, &request, &Default::default())
        .unwrap()
        .claim;
    let original = store.application(app.uuid).unwrap();
    store.access_observer = Some(Arc::new(|_| panic!("query traversed payload")));
    let query = ClaimQuery {
        app: app.uuid,
        conditions: vec![],
    };
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            barrier.wait();
            store
                .set_claim_context(
                    app.uuid,
                    claim.uuid,
                    &[("generation".into(), "new".into())].into(),
                    original.revision,
                )
                .unwrap();
        });
        barrier.wait();
        let snapshot = store.query_claims(&query).unwrap();
        assert_eq!(snapshot.claims[0].claim.entry_uuid, claim.entry_uuid);
        if snapshot.revision == original.revision {
            assert!(snapshot.claims[0].claim.context.is_empty());
        } else {
            assert_eq!(snapshot.revision, original.revision + 1);
            assert_eq!(snapshot.claims[0].claim.context["generation"], "new");
        }
        writer.join().unwrap();
        // Owned snapshots do not follow later mutations.
        assert_eq!(
            snapshot.claims[0].claim.context.is_empty(),
            snapshot.revision == original.revision
        );
    });
    assert_eq!(
        store.query_claims(&query).unwrap().revision,
        original.revision + 1
    );
}
