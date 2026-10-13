use crate::{
    core::{Store, index::Journal},
    models::{
        index::Record,
        policy::PayloadVerification,
        retrieval::{CachePolicy, Ownership, RetrievalPlan, SourceInput},
        source::Source,
        store::Selector,
    },
};
use std::fs;
use uuid::Uuid;

#[test]
fn retrieval_publication_recovers_at_every_boundary_in_both_integrity_modes() {
    for protected in [false, true] {
        for boundary in 0..5 {
            let root = tempfile::tempdir().unwrap();
            let input = root.path().join("input");
            fs::write(&input, b"complete").unwrap();
            let path = root.path().join("root");
            let store = if protected {
                Store::create_fixture(&path).unwrap()
            } else {
                Store::create(&path).unwrap()
            };
            let source = store
                .register_source(Source::Local { path: input })
                .unwrap();
            let request = RetrievalPlan {
                provenance: Vec::new(),
                source: SourceInput::Registered {
                    source_uuid: source.uuid,
                },
                selector: Selector::Local,
                asset: Default::default(),
                cache: CachePolicy::default(),
                verification: PayloadVerification::None,
                expected_sha256: None,
            };
            let resource = store
                .retrieve_prevalidated(&request, &Default::default())
                .unwrap();
            let Ownership::Managed { uuid } = resource.descriptor().ownership else {
                panic!("managed")
            };
            let mut record = store.retrieval_record(uuid).unwrap();
            drop(resource);
            record.uuid = Uuid::new_v4();
            let stage = tempfile::tempdir_in(store.root().join(".staging")).unwrap();
            fs::create_dir(stage.path().join("content")).unwrap();
            fs::write(stage.path().join("content/material"), b"complete").unwrap();
            store.interrupt_commit(
                Journal {
                    records: vec![Record::Retrieval(Box::new(record.clone()))],
                    retrieval_stage: Some((
                        stage.path().file_name().unwrap().to_str().unwrap().into(),
                        record.uuid,
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
            let loaded = reopened.retrieval_record(record.uuid).unwrap();
            assert_eq!(loaded.files, record.files);
            assert_eq!(
                fs::read(reopened.retrieval_path(&loaded).unwrap()).unwrap(),
                b"complete"
            );
            assert_eq!(fs::read_dir(path.join("store")).unwrap().count(), 0);
        }
    }
}
