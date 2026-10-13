use saucepan::{
    core::policy::composition::{effective_revision, validate_composition, validate_retrieval},
    models::{
        policy::{Eviction, Freshness, PayloadVerification, RevisionPolicy, UpdatePolicy},
        retrieval::{CachePolicy, RetrieveRequest},
        store::{AcquisitionRequest, Destination, Request},
    },
};
use serde_json::json;

fn retrieval(cache: CachePolicy) -> RetrieveRequest {
    let mut request: RetrieveRequest = serde_json::from_value(json!({
        "source": {"kind":"inline", "source":{"type":"url", "url":"https://example.invalid/tool.zip"}},
        "selector":{"type":"url"}
    })).unwrap();
    request.cache = cache;
    request
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
            let request = AcquisitionRequest {
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
                let explicit = AcquisitionRequest {
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
    let request = retrieval(CachePolicy::default());
    let mut value = serde_json::to_value(&request).unwrap();
    value["asset"]["folder"] = json!("plugins");
    assert!(serde_json::from_value::<RetrieveRequest>(value).is_err());
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
        freshness: Freshness::CacheOnly {},
        allow_stale: false,
    };
    assert!(validate_composition(&request, &AcquisitionRequest::default(), true).is_err());
    assert!(validate_composition(&request, &AcquisitionRequest::default(), false).is_ok());
    for path in ["../escape", "/absolute", "C:/absolute"] {
        let acquisition = AcquisitionRequest {
            folder: Some(path.into()),
            ..Default::default()
        };
        assert!(validate_composition(&request, &acquisition, false).is_err());
    }
}

#[test]
fn revision_adapter_preserves_current_convenience_defaults() {
    let request: Request = serde_json::from_value(
        json!({"source_uuid":uuid::Uuid::nil(),"selector":{"type":"local"}}),
    )
    .unwrap();
    assert_eq!(
        RevisionPolicy::try_from(&request.policies.eviction).unwrap(),
        RevisionPolicy::Latest { keep: 10 }
    );
    assert_eq!(
        Eviction::try_from(RevisionPolicy::None {}).unwrap(),
        Eviction::Latest { keep: 1 }
    );
    assert!(Eviction::try_from(RevisionPolicy::Latest { keep: 0 }).is_err());
    assert!(RevisionPolicy::try_from(&Eviction::Latest { keep: 0 }).is_err());
    for keep in [1, 3, 10, usize::MAX] {
        let legacy = Eviction::Latest { keep };
        assert_eq!(
            Eviction::try_from(RevisionPolicy::try_from(&legacy).unwrap()).unwrap(),
            legacy
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
    assert_eq!(values.len(), 2);
    let retrieve: RetrieveRequest = serde_json::from_value(values[0].clone()).unwrap();
    let acquire: AcquisitionRequest = serde_json::from_value(values[1].clone()).unwrap();
    validate_composition(&retrieve, &acquire, false).unwrap();
}
