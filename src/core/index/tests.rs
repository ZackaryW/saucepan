use super::*;
use crate::models::{
    index::Binding,
    store::{Request, Selector},
};
fn fixture() -> (tempfile::TempDir, Store, Application, Request) {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("file");
    fs::write(&input, "complete").unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("recovery".into()).unwrap();
    let request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    (tmp, store, app, request)
}
#[test]
fn interrupted_binding_and_deactivation_recover_with_registry_at_each_boundary() {
    for active in [false, true] {
        for boundary in 0..4 {
            let (_tmp, store, mut app, request) = fixture();
            let scope = "fixture".to_string().try_into().unwrap();
            let claim = store
                .acquire_for(app.uuid, &scope, &request, &Default::default())
                .unwrap()
                .claim;
            app = store.application(app.uuid).unwrap();
            let claims_before = store.registry(app.uuid).unwrap().claims;
            app.settings.source_filter = vec![request.source_uuid];
            app.active = active.then(|| Binding {
                entry_uuid: claim.entry_uuid,
                executable: "file".into(),
            });
            app.bump().unwrap();
            store.interrupt_commit(
                Journal {
                    records: vec![Record::Application(app.clone())],
                    ..Default::default()
                },
                boundary,
            );
            let reopened = Store::open_fixture(store.root()).unwrap();
            assert_eq!(
                serde_json::to_value(reopened.application(app.uuid).unwrap()).unwrap(),
                serde_json::to_value(app.clone()).unwrap()
            );
            assert_eq!(
                serde_json::to_value(reopened.registry(app.uuid).unwrap().claims).unwrap(),
                serde_json::to_value(claims_before).unwrap()
            );
            Store::open_fixture(store.root()).unwrap();
        }
    }
}
#[test]
fn interrupted_content_publication_recovers_before_and_after_rename_and_rejects_corruption() {
    for renamed in [false, true] {
        for corrupt in [false, true] {
            let (_tmp, store, app, request) = fixture();
            let scope = "fixture".to_string().try_into().unwrap();
            let old = store
                .acquire_for(app.uuid, &scope, &request, &Default::default())
                .unwrap()
                .claim;
            let mut entry = store.output(old.entry_uuid).unwrap();
            entry.uuid = Uuid::new_v4();
            let stage = tempfile::tempdir_in(store.root.join(".staging")).unwrap();
            fs::create_dir(stage.path().join("content")).unwrap();
            fs::write(stage.path().join("content/file"), "complete").unwrap();
            let (record, registry) = store
                .claim_record(
                    &crate::core::claim::operations::ClaimTarget {
                        app: app.uuid,
                        scope: scope.clone(),
                        id: old.uuid,
                        expected: Some(old.entry_uuid),
                        context: Default::default(),
                    },
                    &request,
                    entry.uuid,
                )
                .unwrap();
            store.interrupt_commit(
                Journal {
                    records: vec![Record::Output(Box::new(entry.clone())), record],
                    registries: vec![registry],
                    stage: Some((
                        stage.path().file_name().unwrap().to_str().unwrap().into(),
                        entry.uuid,
                    )),
                    ..Default::default()
                },
                if renamed { 1 } else { 0 },
            );
            let content = if renamed {
                store.root.join(format!("store/{}", entry.uuid))
            } else {
                stage.path().join("content")
            };
            if corrupt {
                fs::write(content.join("file"), "tampered").unwrap();
                assert!(Store::open_fixture(store.root()).is_err());
                assert_eq!(
                    crate::core::claim::registry::RegistryState::load(
                        &store.metadata,
                        &store.metadata.snapshot().unwrap(),
                        app.uuid
                    )
                    .unwrap()
                    .claims[&old.uuid]
                        .entry_uuid,
                    old.entry_uuid
                );
                assert!(store.output(entry.uuid).is_err());
            } else {
                let reopened = Store::open_fixture(store.root()).unwrap();
                assert_eq!(
                    fs::read(reopened.path(entry.uuid).unwrap().join("file")).unwrap(),
                    b"complete"
                );
                assert_eq!(
                    reopened.registry(app.uuid).unwrap().claims[&old.uuid].entry_uuid,
                    entry.uuid
                );
                Store::open_fixture(store.root()).unwrap();
            }
        }
    }
}
