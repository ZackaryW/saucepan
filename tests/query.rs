mod native_root;
use saucepan::{
    core::query::{ClaimQuery, Condition, Operator},
    models::{
        index::{ClaimContext, Settings},
        policy::Eviction,
        source::Source,
        store::{Availability, Request, Selector},
    },
};
use std::fs;
use uuid::Uuid;

fn setup() -> (tempfile::TempDir, native_root::NativeStore, Uuid, Request) {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, "one").unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    (tmp, store, app.uuid, request)
}
fn condition(key: &str, op: Operator, value: &str) -> Condition {
    Condition {
        key: key.into(),
        op,
        value: value.into(),
    }
}
fn labels(project: &str, alias: &str) -> ClaimContext {
    [
        ("project".into(), project.into()),
        ("source.alias".into(), alias.into()),
    ]
    .into()
}

#[test]
fn searches_one_application_with_and_filters_and_deterministic_duplicates() {
    let (_tmp, store, app, request) = setup();
    let mut expected = vec![];
    for (ordinal, alias) in ["skills-core", "skills-extra", "skills-core"]
        .into_iter()
        .enumerate()
    {
        expected.push(
            store
                .acquire_for_with_context(
                    app,
                    &format!("project-a/{ordinal}").try_into().unwrap(),
                    &request,
                    &labels("A", alias),
                    &Default::default(),
                )
                .unwrap()
                .claim
                .uuid,
        );
    }
    store
        .acquire_for_with_context(
            app,
            &"project-b".to_string().try_into().unwrap(),
            &request,
            &labels("B", "skills-core"),
            &Default::default(),
        )
        .unwrap();
    store
        .acquire_for(
            app,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    let other = store.register_app("other".into()).unwrap();
    store
        .acquire_for_with_context(
            other.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &labels("A", "skills-core"),
            &Default::default(),
        )
        .unwrap();
    let query = ClaimQuery {
        scope: None,
        app,
        conditions: vec![
            condition("project", Operator::Eq, "A"),
            condition("source.alias", Operator::Glob, "skills-*"),
        ],
    };
    let found = store.query_claims(&query).unwrap();
    expected.sort();
    assert_eq!(
        found
            .claims
            .iter()
            .map(|v| v.claim.uuid)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(found.app, app);
    assert_eq!(found.revision, store.application(app).unwrap().revision);
    assert!(found.claims.iter().all(|v| v.state == Availability::Ready));
    let exact_star = ClaimQuery {
        scope: None,
        app,
        conditions: vec![condition("source.alias", Operator::Eq, "skills-*")],
    };
    assert!(store.query_claims(&exact_star).unwrap().claims.is_empty());
    let missing = ClaimQuery {
        scope: None,
        app,
        conditions: vec![condition("missing", Operator::Glob, "*")],
    };
    assert!(store.query_claims(&missing).unwrap().claims.is_empty());
    let all = ClaimQuery {
        scope: None,
        app,
        conditions: vec![],
    };
    assert_eq!(store.query_claims(&all).unwrap().claims.len(), 5);
    assert_eq!(
        serde_json::to_value(store.query_claims(&all).unwrap().claims).unwrap(),
        serde_json::to_value(store.claims(app).unwrap()).unwrap()
    );
    let unrelated = store
        .register_source(Source::Local {
            path: store.root().to_owned(),
        })
        .unwrap();
    store
        .set_settings(
            app,
            Settings {
                source_filter: vec![unrelated.uuid],
            },
        )
        .unwrap();
    assert!(store.query_claims(&all).unwrap().claims.is_empty());
    assert!(store.query_claims(&query).unwrap().claims.is_empty());
    assert!(
        store
            .query_claims(&ClaimQuery {
                scope: None,
                app: request.source_uuid,
                conditions: vec![]
            })
            .is_err()
    );
}

#[test]
fn query_uses_metadata_without_refreshing_sources_or_touching_payloads() {
    let (_tmp, store, app, request) = setup();
    let output = store
        .acquire_for_with_context(
            app,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &labels("A", "skills-core"),
            &Default::default(),
        )
        .unwrap();
    let app_path = store.root().join(format!("index/{app}/index.json"));
    let before = fs::read(&app_path).unwrap();
    // Source and output bytes may be unavailable; signed provenance must remain intact.
    fs::remove_file(_tmp.path().join("file")).unwrap();
    fs::remove_file(output.acquired.directory.join("file")).unwrap();
    let query = ClaimQuery {
        scope: None,
        app,
        conditions: vec![],
    };
    assert_eq!(
        store.query_claims(&query).unwrap().claims[0].state,
        Availability::Ready
    );
    assert_eq!(fs::read(app_path).unwrap(), before);
    let path = store
        .root()
        .join(format!("index/{}/index.json", output.claim.entry_uuid));
    fs::write(&path, "broken").unwrap();
    assert!(store.query_claims(&query).is_err());
    fs::remove_file(&path).unwrap();
    assert!(store.query_claims(&query).is_err());
    let no_match = ClaimQuery {
        scope: None,
        app,
        conditions: vec![condition("project", Operator::Eq, "elsewhere")],
    };
    assert!(store.query_claims(&no_match).is_err());
}

#[test]
fn results_do_not_protect_content_and_unavailable_claims_remain_searchable() {
    let (tmp, store, app, mut request) = setup();
    request.policies.eviction = Eviction::Latest { keep: 1 };
    let old = store
        .acquire_for_with_context(
            app,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &labels("A", "skills-core"),
            &Default::default(),
        )
        .unwrap();
    let query = ClaimQuery {
        scope: None,
        app,
        conditions: vec![],
    };
    let snapshot = store.query_claims(&query).unwrap();
    fs::write(tmp.path().join("file"), "two").unwrap();
    store.acquire(&request).unwrap();
    assert!(!old.acquired.directory.exists());
    assert_eq!(snapshot.claims[0].state, Availability::Ready);
    let after = store.query_claims(&query).unwrap();
    assert_eq!(after.claims[0].state, Availability::Reclaimed);
    assert_eq!(after.claims[0].claim.context, labels("A", "skills-core"));
    assert_eq!(after.revision, snapshot.revision);
    assert!(
        store
            .lease(old.claim.entry_uuid, Default::default())
            .is_err()
    );
}
