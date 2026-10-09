use super::*;
use crate::models::{source::Source, store::Selector};
use std::{fs, sync::Arc};

#[test]
fn primitive_commit_keeps_intervening_context_and_returns_that_snapshot() {
    for reuse in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "one").unwrap();
        let mut store = Store::create(&tmp.path().join("root")).unwrap();
        let app = store.register_app("test".into()).unwrap();
        let source = store
            .register_source(Source::Local { path: file.clone() })
            .unwrap();
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
        if !reuse {
            fs::write(file, "two").unwrap();
        }
        let editor = store.clone();
        let labels: ClaimContext = [("project".into(), "edited-during-prepare".into())].into();
        let edit = labels.clone();
        store.before_claim_commit = Some(Arc::new(move || {
            let revision = editor.application(app.uuid).unwrap().revision;
            editor
                .set_claim_context(app.uuid, claim.uuid, &edit, revision)
                .unwrap();
        }));
        let updated = store
            .update_claim(app.uuid, claim.uuid, &Default::default())
            .unwrap();
        assert_eq!(updated.acquired.reused, reuse);
        assert_eq!(updated.claim.context, labels);
        assert_eq!(
            serde_json::to_value(&updated.claim).unwrap(),
            serde_json::to_value(&store.application(app.uuid).unwrap().claims[&claim.uuid])
                .unwrap()
        );
    }
}

#[test]
fn context_journal_recovers_before_and_after_record_publication() {
    for published in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create(&tmp.path().join("root")).unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "one").unwrap();
        let source = store.register_source(Source::Local { path: file }).unwrap();
        let mut app = store.register_app("test".into()).unwrap();
        let request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policies: Default::default(),
        };
        let claimed = store
            .acquire_for(app.uuid, &request, &Default::default())
            .unwrap();
        app = store.application(app.uuid).unwrap();
        app.claims.get_mut(&claimed.claim.uuid).unwrap().context =
            [("project".into(), "recovered".into())].into();
        app.bump().unwrap();
        let record = Record::Application(app.clone());
        super::super::write(
            &store.root.join(".staging/transaction.json"),
            &Journal {
                records: vec![record.clone()],
                ..Default::default()
            },
        )
        .unwrap();
        if published {
            store.save_record(&record).unwrap();
        }
        let reopened = Store::open(&store.root).unwrap();
        assert_eq!(
            serde_json::to_value(reopened.application(app.uuid).unwrap()).unwrap(),
            serde_json::to_value(&app).unwrap()
        );
        assert!(!store.root.join(".staging/transaction.json").exists());
    }
}
