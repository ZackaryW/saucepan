use saucepan::{
    core::policy::composition::{effective_revision, validate_composition, validate_retrieval},
    models::{
        policy::{Freshness, PayloadVerification, RevisionPolicy, UpdatePolicy},
        retrieval::{CachePolicy, RetrievalPlan},
        store::{AcquisitionPlan, Destination, Request},
    },
};
use serde_json::json;

fn retrieval(cache: CachePolicy) -> RetrievalPlan {
    RetrievalPlan {
        provenance: vec![],
        source: saucepan::models::retrieval::SourceInput::Inline {
            source: saucepan::models::source::Source::Url {
                url: "https://example.invalid/tool.zip".into(),
            },
        },
        selector: saucepan::models::store::Selector::Url,
        asset: Default::default(),
        cache,
        verification: Default::default(),
        expected_sha256: None,
    }
}

#[test]
fn persisted_retrieval_evidence_does_not_fill_missing_fields() {
    let value = serde_json::to_value(retrieval(CachePolicy::default())).unwrap();
    for field in ["provenance", "asset", "cache", "verification"] {
        let mut incomplete = value.clone();
        incomplete.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<RetrievalPlan>(incomplete).is_err(),
            "{field}"
        );
    }
}

#[test]
fn destination_matrix_rejects_implicit_managed_effects() {
    for cache in [
        CachePolicy::default(),
        CachePolicy::Temporary {},
        CachePolicy::NoStore {
            destination: "source".into(),
        },
    ] {
        for destination in [
            Destination::Managed {},
            Destination::Temporary {},
            Destination::Directory {
                path: "output".into(),
            },
        ] {
            let request = AcquisitionPlan {
                destination,
                ..Default::default()
            };
            let managed = matches!(request.destination, Destination::Managed {});
            assert_eq!(
                validate_composition(&retrieval(cache.clone()), &request, false).is_ok(),
                !managed || matches!(cache, CachePolicy::Cached { .. })
            );
            assert_eq!(
                effective_revision(&request).unwrap(),
                managed.then(RevisionPolicy::default)
            );
            if !managed {
                let explicit = AcquisitionPlan {
                    revision_policy: Some(RevisionPolicy::None {}),
                    ..request
                };
                assert!(validate_composition(&retrieval(cache.clone()), &explicit, false).is_err());
            }
        }
    }
}

#[test]
fn new_dtos_cannot_smuggle_legacy_or_cross_stage_options() {
    use saucepan::models::{retrieval::RetrieveRequest, store::AcquisitionRequest};
    for extra in [
        json!({"eviction":{"strategy":"latest","keep":2}}),
        json!({"policies":{}}),
        json!({"filters":[]}),
    ] {
        assert!(serde_json::from_value::<AcquisitionRequest>(extra).is_err());
    }
    for cache in [
        json!({"mode":"no_store"}),
        json!({"mode":"temporary","freshness":{"mode":"cache_only"}}),
        json!({"mode":"no_store","destination":"out","allow_stale":true}),
    ] {
        assert!(serde_json::from_value::<CachePolicy>(cache).is_err());
    }
    assert!(
        serde_json::from_value::<Freshness>(json!({"mode":"cache_only", "seconds":1})).is_err()
    );
    assert!(serde_json::from_value::<UpdatePolicy>(json!({"mode":"manual", "seconds":1})).is_err());
    assert!(serde_json::from_value::<RevisionPolicy>(json!({"history":"none", "keep":3})).is_err());
    assert!(
        serde_json::from_value::<Destination>(json!({"mode":"managed", "path":"out"})).is_err()
    );
    let value = json!({"source":{"kind":"inline","source":{"type":"local","path":"input"}}, "selector":{"type":"local"}, "policy_refs":[], "asset":{"filter_refs":[]}});
    let request: RetrieveRequest = serde_json::from_value(value.clone()).unwrap();
    for (field, extra) in [
        ("cache", json!({"mode":"temporary"})),
        ("verification", json!("none")),
        ("provenance", json!([])),
    ] {
        let mut mixed = value.clone();
        mixed[field] = extra;
        assert!(serde_json::from_value::<RetrieveRequest>(mixed).is_err());
    }
    for field in [
        "identical",
        "update_policy",
        "revision_policy",
        "protection",
        "adopted",
        "evaluated",
        "provenance",
    ] {
        let mut mixed = json!({"policy_refs":[]});
        mixed[field] = json!(null);
        assert!(
            serde_json::from_value::<AcquisitionRequest>(mixed).is_err(),
            "{field}"
        );
    }
    assert_eq!(
        serde_json::from_value::<RetrieveRequest>(serde_json::to_value(&request).unwrap()).unwrap(),
        request
    );
}

#[test]
fn contradictions_fail_without_io() {
    let mut request = retrieval(CachePolicy::NoStore {
        destination: "".into(),
    });
    assert!(validate_retrieval(&request).is_err());
    request.cache = CachePolicy::Temporary {};
    request.verification = PayloadVerification::None;
    request.expected_sha256 = Some("a".repeat(64));
    assert!(validate_retrieval(&request).is_err());
    request.verification = PayloadVerification::Required;
    assert!(validate_retrieval(&request).is_ok());
    request.expected_sha256 = Some("invalid".into());
    assert!(validate_retrieval(&request).is_err());
    request.expected_sha256 = None;
    request.cache = CachePolicy::Cached {
        freshness: Some(Freshness::CacheOnly {}),
        allow_stale: false,
    };
    assert!(validate_composition(&request, &AcquisitionPlan::default(), true).is_err());
    assert!(validate_composition(&request, &AcquisitionPlan::default(), false).is_ok());
    for path in ["../escape", "/absolute", "C:/absolute"] {
        let acquisition = AcquisitionPlan {
            folder: Some(path.into()),
            ..Default::default()
        };
        assert!(validate_composition(&request, &acquisition, false).is_err());
    }
}

#[test]
fn revision_policy_preserves_history_choice_and_defaults() {
    let request: Request = serde_json::from_value(
        json!({"source_uuid":uuid::Uuid::nil(),"selector":{"type":"local"}}),
    )
    .unwrap();
    assert!(request.policy_refs.ids().is_empty());
    assert_eq!(
        saucepan::models::policy::effective::Behavior::default().revision,
        RevisionPolicy::Latest { keep: 10 }
    );
    assert_eq!(RevisionPolicy::None {}.keep().unwrap(), 1);
    assert!(RevisionPolicy::Latest { keep: 0 }.keep().is_err());
    for policy in [
        RevisionPolicy::None {},
        RevisionPolicy::Latest { keep: 1 },
        RevisionPolicy::Latest { keep: 10 },
    ] {
        assert_eq!(
            serde_json::from_value::<RevisionPolicy>(serde_json::to_value(policy).unwrap())
                .unwrap(),
            policy
        );
    }
}

#[test]
fn update_due_uses_success_time_and_handles_clock_rollback() {
    let policy = UpdatePolicy::MaxAge { seconds: 60 };
    assert!(!policy.due(Some(100), 159));
    assert!(policy.due(Some(100), 160));
    assert!(policy.due(Some(100), 99));
    assert!(policy.due(None, 0));
    assert!(!UpdatePolicy::Manual {}.due(None, 999));
    assert!(UpdatePolicy::Always {}.due(Some(999), 999));
}

#[test]
fn documented_request_examples_parse_and_validate() {
    let documentation = include_str!("../docs/composition.md");
    let values: Vec<serde_json::Value> = documentation
        .split("```json\n")
        .skip(1)
        .map(|part| serde_json::from_str(part.split("```").next().unwrap()).unwrap())
        .collect();
    use saucepan::{
        core::policy::{
            catalog::PolicyCatalog,
            prepare::{self, Attachments},
        },
        models::policy::requests::{Acquisition, Retrieval},
    };
    assert_eq!(values.len(), 7);
    let catalog = PolicyCatalog::new(
        serde_json::from_value::<Vec<saucepan::models::policy::PolicyRecord>>(values[0].clone())
            .unwrap(),
    )
    .unwrap();
    let empty = Default::default();
    let attachments = Attachments {
        source: &empty,
        application: &empty,
    };
    for index in [1, 3] {
        let input: Retrieval = serde_json::from_value(values[index].clone()).unwrap();
        prepare::retrieval(&catalog, &attachments, &input).unwrap();
    }
    for index in [2, 4] {
        let input: Acquisition = serde_json::from_value(values[index].clone()).unwrap();
        prepare::acquisition(&catalog, &attachments, &input).unwrap();
    }
    let _: Acquisition = serde_json::from_value(values[5]["acquisition"].clone()).unwrap();
    let retrieval: Retrieval = serde_json::from_value(values[6]["retrieval"].clone()).unwrap();
    let acquisition: Acquisition =
        serde_json::from_value(values[6]["acquisition"].clone()).unwrap();
    prepare::retrieval(&catalog, &attachments, &retrieval).unwrap();
    prepare::acquisition(&catalog, &attachments, &acquisition).unwrap();
}
