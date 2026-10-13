use super::*;
use crate::models::store::VerificationMode;
use crate::{
    core::index::Journal,
    models::{source::Source, store::Selector},
};
use std::fs;

#[test]
fn acquisition_selection_and_output_recover_together_in_each_integrity_mode() {
    for protected in [false, true] {
        for boundary in 0..5 {
            let root = tempfile::tempdir().unwrap();
            let input = root.path().join("input");
            fs::write(&input, b"payload").unwrap();
            let path = root.path().join("root");
            let store = if protected {
                Store::create_fixture(&path).unwrap()
            } else {
                Store::create(&path).unwrap()
            };
            let source = store
                .register_source(Source::Local { path: input })
                .unwrap();
            let retrieval = RetrieveRequest {
                provenance: Vec::new(),
                source: SourceInput::Registered {
                    source_uuid: source.uuid,
                },
                selector: Selector::Local,
                asset: Default::default(),
                cache: CachePolicy::default(),
                verification: Default::default(),
                expected_sha256: None,
            };
            let acquisition = AcquisitionRequest {
                update_policy: UpdatePolicy::Always {},
                ..Default::default()
            };
            let first = store.acquire_composed(&retrieval, &acquisition).unwrap();
            let key = intent_key(&retrieval, &acquisition).unwrap();
            let mut state = store.acquisition_by_key(&key).unwrap().unwrap();
            let mut output = store.output(state.selected).unwrap();
            let stage = tempfile::tempdir_in(path.join(".staging")).unwrap();
            crate::utils::tree::copy_tree_unhashed(
                first.path(),
                &stage.path().join("content"),
                Default::default(),
            )
            .unwrap();
            output.uuid = Uuid::new_v4();
            output.sequence += 1;
            state.selected = output.uuid;
            state.revision += 1;
            state.last_success = Some(state.last_success.unwrap() + 1);
            store.interrupt_commit(
                Journal {
                    records: vec![
                        Record::Output(Box::new(output.clone())),
                        Record::Acquisition(Box::new(state.clone())),
                    ],
                    stage: Some((
                        stage.path().file_name().unwrap().to_str().unwrap().into(),
                        output.uuid,
                    )),
                    ..Default::default()
                },
                boundary,
            );
            let reopened = if protected {
                Store::open_fixture(&path).unwrap()
            } else {
                Store::open(&path).unwrap()
            };
            let loaded = reopened.acquisition_by_key(&key).unwrap().unwrap();
            assert_eq!(loaded.selected, output.uuid);
            assert_eq!(loaded.revision, state.revision);
            assert_eq!(loaded.last_success, state.last_success);
            let lease = reopened
                .lease(loaded.selected, VerificationMode::Full)
                .unwrap();
            assert_eq!(
                fs::read(lease.directory().join("input")).unwrap(),
                b"payload"
            );
        }
    }
}
