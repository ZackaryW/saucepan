use super::*;
use crate::{
    models::{
        source::Source,
        store::{Request, Selector},
    },
    utils::tree::IoEvent,
};
use serde_json::json;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[test]
fn verification_bridges_reclaim_and_fences_source_edits() {
    for edit_policy in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = Store::create_fixture(&tmp.path().join("root")).unwrap();
        let input = tmp.path().join("input");
        fs::write(&input, "one").unwrap();
        let source = store
            .register_source(Source::Local {
                path: input.clone(),
            })
            .unwrap();
        let refs = crate::policy_fixture::definitions(
            &store,
            vec![json!({"type":"verification_policy","policy_target":"path","mode":"full"})],
        )
        .unwrap();
        store
            .adopt_source_policies(source.uuid, source.policy_revision, refs)
            .unwrap();
        let request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policy_refs: crate::policy_fixture::definitions(
                &store,
                vec![json!({"type":"revision_policy","history":"none"})],
            )
            .unwrap(),
        };
        let output = store.acquire(&request).unwrap().entry.uuid;
        let pin = PinRequest {
            registration_key: "key".to_owned().try_into().unwrap(),
            outputs: vec![output],
            policy_refs: crate::policy_fixture::definitions(
                &store,
                vec![json!({"type":"pin_policy","retention":"until_released"})],
            )
            .unwrap(),
        };
        let editor = store.clone();
        let once = AtomicBool::new(false);
        store.access_observer = Some(Arc::new(move |event| {
            if !matches!(event, IoEvent::Open) || once.swap(true, Ordering::SeqCst) {
                return;
            }
            if edit_policy {
                let source = editor.source(source.uuid).unwrap();
                editor
                    .adopt_source_policies(source.uuid, source.policy_revision, Default::default())
                    .unwrap();
            } else {
                fs::write(&input, "two").unwrap();
                editor.acquire(&request).unwrap();
                assert!(editor.reclaim().unwrap().deferred.contains_key(&output));
            }
        }));
        let result = store.pin_outputs(&pin);
        assert_eq!(result.is_err(), edit_policy);
        if edit_policy {
            assert!(store.pins(None).unwrap().is_empty());
        } else {
            assert!(store.reclaim().unwrap().deferred.contains_key(&output));
        }
    }
}
