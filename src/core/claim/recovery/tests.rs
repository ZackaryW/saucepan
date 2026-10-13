use super::*;
use crate::models::{
    claim::{
        CompiledEnsure as EnsureRequest,
        policy::{ReclaimPolicy, ReclaimStrategy},
    },
    policy::Identical,
    source::Source,
    store::Selector,
};
use std::{fs, sync::Arc};

#[test]
fn recovery_keeps_newer_state_and_rejects_dependency_mismatch_before_publication() {
    for action in ["context", "release", "dependencies"] {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let input = tmp.path().join("file");
        fs::write(&input, "original").unwrap();
        let source = store
            .register_source(Source::Local { path: input })
            .unwrap();
        let app = store.register_app("test".into()).unwrap().uuid;
        let mut request = EnsureRequest {
            adopted: None,
            scope: "scope".to_string().try_into().unwrap(),
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            context: Default::default(),
            overrides: Default::default(),
        };
        request.overrides.revision =
            Some(saucepan::models::policy::RevisionPolicy::Latest { keep: 1 });
        request.overrides.reclaim = Some(ReclaimPolicy::ReclaimPolicy {
            strategy: ReclaimStrategy::RestoreExact,
        });
        let first = store
            .ensure_compiled(app, &request, &Default::default())
            .unwrap();
        let mut pull = first.claim.request.clone();
        pull.policies.identical = Identical::NewEntry;
        store.acquire(&pull).unwrap();
        let id = first.claim.uuid;
        if action == "dependencies" {
            let mut output = store.output(first.claim.entry_uuid).unwrap();
            output
                .dependencies
                .insert("submodule".into(), "original-object".into());
            let _lock = store.lock().unwrap();
            store
                .commit(Journal {
                    records: vec![Record::Output(Box::new(output))],
                    ..Default::default()
                })
                .unwrap();
        } else {
            let writer = store.clone();
            store.before_claim_commit = Some(Arc::new(move || {
                if action == "release" {
                    writer.release_claim(app, id).unwrap();
                } else {
                    writer
                        .set_claim_context(
                            app,
                            id,
                            &[("changed".into(), "yes".into())].into(),
                            writer.application(app).unwrap().revision,
                        )
                        .unwrap();
                }
                // The prepared candidate stays leased even under competing reclaim.
                writer.reclaim().unwrap();
            }));
        }
        let error = store
            .recover_claim(app, id, &Default::default())
            .unwrap_err();
        assert_eq!(
            Error::from_anyhow(&error).code,
            if action == "dependencies" {
                ErrorCode::IntegrityError
            } else {
                ErrorCode::RevisionConflict
            }
        );
        let registry = store.registry(app).unwrap();
        if action == "release" {
            assert!(registry.claims.is_empty());
        } else {
            assert_eq!(registry.claims[&id].entry_uuid, first.claim.entry_uuid);
            if action == "context" {
                assert_eq!(registry.claims[&id].context["changed"], "yes");
            }
        }
    }
}
