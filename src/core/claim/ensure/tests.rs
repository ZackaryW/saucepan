use super::*;
use crate::{
    core::claim::session::ApplicationConfiguration,
    models::{claim::policy::Action, source::Source, store::Selector},
};
use std::{fs, sync::Arc};

#[test]
fn intervening_writers_cannot_be_overwritten_by_new_entry_ensure() {
    for action in ["release", "context", "policy", "update", "uninstall"] {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("file");
        fs::write(&file, "original").unwrap();
        let mut store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let source = store
            .register_source(Source::Local { path: file.clone() })
            .unwrap();
        let app = store.register_app("consumer".into()).unwrap().uuid;
        let mut request = EnsureRequest {
            scope: "project".to_string().try_into().unwrap(),
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            context: Default::default(),
            overrides: Default::default(),
        };
        let first = store
            .ensure_claim(app, &request, &Default::default())
            .unwrap();
        let writer = store.clone();
        let id = first.claim.uuid;
        request.overrides.identical = Some(Identical::NewEntry);
        store.before_claim_commit = Some(Arc::new(move || match action {
            "release" => writer.release_claim(app, id).unwrap(),
            "context" => {
                writer
                    .set_claim_context(
                        app,
                        id,
                        &[("new".into(), "context".into())].into(),
                        writer.application(app).unwrap().revision,
                    )
                    .unwrap();
            }
            "policy" => {
                let mut access = writer.registry(app).unwrap().registry.access;
                access.actions = Some([Action::Read, Action::Discover].into());
                writer
                    .configure_application(
                        app,
                        ApplicationConfiguration {
                            expected_revision: writer.application(app).unwrap().revision,
                            access,
                            policies: Default::default(),
                            credential: None,
                        },
                        None,
                    )
                    .unwrap();
            }
            "update" => {
                fs::write(&file, "newer writer").unwrap();
                writer.update_claim(app, id, &Default::default()).unwrap();
            }
            "uninstall" => {
                writer.uninstall(app, false, None).unwrap();
            }
            _ => unreachable!(),
        }));
        let result = store.ensure_claim(app, &request, &Default::default());
        let code = Error::from_anyhow(&result.unwrap_err()).code;
        assert!(
            matches!(
                code,
                ErrorCode::RevisionConflict
                    | ErrorCode::ClaimContextConflict
                    | ErrorCode::AccessDenied
                    | ErrorCode::UnknownApplication
            ),
            "{action}: {code:?}"
        );
        if action == "uninstall" {
            continue;
        }
        let registry = store.registry(app).unwrap();
        if action == "release" {
            assert!(registry.claims.is_empty());
        } else {
            assert_eq!(registry.claims.len(), 1);
            let current = &registry.claims[&id];
            if action == "context" {
                assert_eq!(current.context["new"], "context");
            }
            if action == "update" {
                assert_eq!(
                    fs::read(store.path(current.entry_uuid).unwrap().join("file")).unwrap(),
                    b"newer writer"
                );
            } else {
                assert_eq!(current.entry_uuid, first.claim.entry_uuid);
            }
        }
    }
}
