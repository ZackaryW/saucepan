use super::*;
use crate::models::store::Request;
use crate::models::{source::Source, store::Selector};
use std::{fs, sync::Arc};

#[test]
fn primitive_commit_fences_intervening_context_and_preserves_selection() {
    for reuse in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "one").unwrap();
        let mut store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let app = store.register_app("test".into()).unwrap();
        let source = store
            .register_source(Source::Local { path: file.clone() })
            .unwrap();
        let request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policy_refs: Default::default(),
        };
        let claim = store
            .acquire_for(
                app.uuid,
                &"fixture".to_string().try_into().unwrap(),
                &request,
                &Default::default(),
            )
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
        let error = store
            .update_claim(app.uuid, claim.uuid, &Default::default())
            .unwrap_err();
        assert_eq!(Error::from_anyhow(&error).code, ErrorCode::RevisionConflict);
        let current = store.registry(app.uuid).unwrap();
        assert_eq!(current.claims[&claim.uuid].context, labels);
        assert_eq!(current.claims[&claim.uuid].entry_uuid, claim.entry_uuid);
    }
}

#[test]
fn context_journal_recovers_before_and_after_record_publication() {
    for published in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "one").unwrap();
        let source = store.register_source(Source::Local { path: file }).unwrap();
        let mut app = store.register_app("test".into()).unwrap();
        let request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policy_refs: Default::default(),
        };
        let claimed = store
            .acquire_for(
                app.uuid,
                &"fixture".to_string().try_into().unwrap(),
                &request,
                &Default::default(),
            )
            .unwrap();
        app = store.application(app.uuid).unwrap();
        let mut registry = store.registry(app.uuid).unwrap();
        registry
            .claims
            .get_mut(&claimed.claim.uuid)
            .unwrap()
            .context = [("project".into(), "recovered".into())].into();
        app.bump().unwrap();
        let record = Record::Application(app.clone());
        store.interrupt_commit(
            Journal {
                records: vec![record.clone()],
                registries: vec![registry],
                ..Default::default()
            },
            if published { 1 } else { 0 },
        );
        let reopened = Store::open_fixture(&store.root).unwrap();
        assert_eq!(
            reopened.registry(app.uuid).unwrap().claims[&claimed.claim.uuid].context["project"],
            "recovered"
        );
        assert_eq!(
            serde_json::to_value(reopened.application(app.uuid).unwrap()).unwrap(),
            serde_json::to_value(&app).unwrap()
        );
        assert!(
            !store
                .root
                .join(".staging/metadata-transaction.json")
                .exists()
        );
    }
}
