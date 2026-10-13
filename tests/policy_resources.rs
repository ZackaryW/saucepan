use saucepan::{
    core::policy::{
        catalog::PolicyCatalog,
        resolve::{self, Inputs, Operation},
    },
    error::ErrorCode,
    models::{
        claim::{Scope, policy::Action},
        policy::{
            PolicyRecord, PolicyRefs, RevisionPolicy, UpdatePolicy, effective::EvaluatedPolicies,
        },
    },
};
use serde_json::{Value, json};
use uuid::Uuid;

fn record(mut value: Value) -> PolicyRecord {
    value["uuid"] = json!(Uuid::new_v4());
    serde_json::from_value(value).unwrap()
}
fn refs(records: &[&PolicyRecord]) -> PolicyRefs {
    records
        .iter()
        .map(|r| r.uuid)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
fn evaluate(
    catalog: &PolicyCatalog,
    source: &PolicyRefs,
    app: &PolicyRefs,
    request: &PolicyRefs,
    pinned: Option<&EvaluatedPolicies>,
) -> Result<EvaluatedPolicies, saucepan::error::Error> {
    resolve::evaluate(
        catalog,
        Inputs {
            source,
            application: app,
            request,
            pinned,
            operation: Operation::Compose,
            managed_output: true,
            update_due: false,
        },
    )
}

#[test]
fn flat_definitions_round_trip_and_reject_unknown_recursive_and_duplicate_inputs() {
    let examples = [
        json!({"type":"access_policy","scopes":["library:main"]}),
        json!({"type":"source_visibility_policy","public_discovery":true}),
        json!({"type":"claim_visibility_policy","public_scopes":[]}),
        json!({"type":"cache_policy","mode":"temporary"}),
        json!({"type":"refresh_policy","mode":"max_age","seconds":86400}),
        json!({"type":"stale_policy","allow_stale":true}),
        json!({"type":"verification_policy","policy_target":"payload","mode":"required"}),
        json!({"type":"verification_policy","policy_target":"path","mode":"full"}),
        json!({"type":"github_release_asset_filter","platform":null,"asset_patterns":["*.zip"]}),
        json!({"type":"selection_policy","policy_target":"github_release_asset_filter","strategy":"first_match_win"}),
        json!({"type":"identical_pull_policy","strategy":"new_entry"}),
        json!({"type":"update_policy","mode":"always"}),
        json!({"type":"revision_policy","history":"latest","keep":3}),
        json!({"type":"reclaim_policy","strategy":"restore_exact"}),
        json!({"type":"useable_app_policy"}),
        json!({"type":"policy_constraints","minimum_keep":2,"maximum_keep":5}),
    ];
    for value in examples {
        let policy = record(value);
        let encoded = serde_json::to_value(&policy).unwrap();
        assert!(encoded.get("definition").is_none());
        assert_eq!(
            serde_json::from_value::<PolicyRecord>(encoded.clone()).unwrap(),
            policy
        );
        for field in ["unknown", "policy_refs", "defaults", "overrides"] {
            let mut bad = encoded.clone();
            bad[field] = json!([]);
            assert!(
                serde_json::from_value::<PolicyRecord>(bad).is_err(),
                "accepted {field} for {encoded}"
            );
        }
    }
    let policy = record(json!({"type":"revision_policy","history":"latest","keep":3}));
    assert!(PolicyCatalog::new([policy.clone(), policy.clone()]).is_err());
    assert!(PolicyRefs::try_from(vec![policy.uuid, policy.uuid]).is_err());
    assert!(PolicyRefs::try_from(vec![Uuid::new_v4(); 257]).is_err());
    let bad = record(json!({"type":"revision_policy","history":"latest","keep":0}));
    assert!(PolicyCatalog::new([bad]).is_err());
}

#[test]
fn precedence_provenance_and_pinned_claims_do_not_reinterpret_defaults() {
    let source = record(json!({"type":"revision_policy","history":"latest","keep":3}));
    let app = record(json!({"type":"revision_policy","history":"latest","keep":5}));
    let request = record(json!({"type":"revision_policy","history":"latest","keep":7}));
    let catalog = PolicyCatalog::new([source.clone(), app.clone(), request.clone()]).unwrap();
    let empty = PolicyRefs::default();
    let defaults = evaluate(&catalog, &empty, &empty, &empty, None).unwrap();
    assert_eq!(
        defaults.behavior.revision,
        RevisionPolicy::Latest { keep: 10 }
    );
    assert_eq!(defaults.behavior.update, UpdatePolicy::Manual {});
    assert!(defaults.evidence.is_empty());
    let selected = evaluate(&catalog, &refs(&[&source]), &refs(&[&app]), &empty, None).unwrap();
    assert_eq!(
        selected.behavior.revision,
        RevisionPolicy::Latest { keep: 5 }
    );
    let explicit = evaluate(
        &catalog,
        &refs(&[&source]),
        &refs(&[&app]),
        &refs(&[&request]),
        None,
    )
    .unwrap();
    assert_eq!(
        explicit.behavior.revision,
        RevisionPolicy::Latest { keep: 7 }
    );
    let pinned = evaluate(
        &catalog,
        &refs(&[&source]),
        &refs(&[&request]),
        &empty,
        Some(&selected),
    )
    .unwrap();
    assert_eq!(pinned.behavior.revision, selected.behavior.revision);
    assert_eq!(pinned.evidence, selected.evidence);
    assert_eq!(
        evaluate(&catalog, &empty, &refs(&[&app, &source]), &empty, None)
            .unwrap_err()
            .code,
        ErrorCode::PolicyConflict
    );
}

#[test]
fn current_restrictions_intersect_and_requests_cannot_grant_authority() {
    let scope = Scope::try_from("library:main".to_owned()).unwrap();
    let permitted = record(
        json!({"type":"access_policy","actions":["read","acquire"],"scopes":["library:main"]}),
    );
    let denied = record(json!({"type":"access_policy","actions":[]}));
    let catalog = PolicyCatalog::new([permitted.clone(), denied.clone()]).unwrap();
    let empty = PolicyRefs::default();
    let plan = evaluate(&catalog, &empty, &refs(&[&permitted]), &empty, None).unwrap();
    resolve::authorize(&plan, Action::Read, None, Some(&scope)).unwrap();
    assert_eq!(
        resolve::authorize(&plan, Action::Read, None, None)
            .unwrap_err()
            .code,
        ErrorCode::AccessDenied
    );
    let restricted = evaluate(
        &catalog,
        &empty,
        &refs(&[&permitted, &denied]),
        &empty,
        Some(&plan),
    )
    .unwrap();
    assert_eq!(
        resolve::authorize(&restricted, Action::Read, None, Some(&scope))
            .unwrap_err()
            .code,
        ErrorCode::AccessDenied
    );
    assert_eq!(
        evaluate(&catalog, &empty, &empty, &refs(&[&permitted]), None)
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn constraints_and_stage_conflicts_fail_in_pure_preflight() {
    let bounds = record(
        json!({"type":"policy_constraints","minimum_keep":2,"maximum_keep":5,"require_useable_app":true,"minimum_update_interval":60}),
    );
    let revision = record(json!({"type":"revision_policy","history":"latest","keep":3}));
    let fast = record(json!({"type":"update_policy","mode":"always"}));
    let fresh = record(json!({"type":"cache_policy","mode":"temporary"}));
    let stale = record(json!({"type":"stale_policy","allow_stale":true}));
    let offline = record(json!({"type":"refresh_policy","mode":"cache_only"}));
    let catalog = PolicyCatalog::new([
        bounds.clone(),
        revision.clone(),
        fast.clone(),
        fresh.clone(),
        stale.clone(),
        offline.clone(),
    ])
    .unwrap();
    let empty = PolicyRefs::default();
    let plan = evaluate(&catalog, &empty, &refs(&[&bounds, &revision]), &empty, None).unwrap();
    assert!(plan.behavior.useable_app);
    assert_eq!(
        evaluate(
            &catalog,
            &empty,
            &refs(&[&bounds, &revision]),
            &refs(&[&fast]),
            None
        )
        .unwrap_err()
        .code,
        ErrorCode::PolicyConflict
    );
    for request in [refs(&[&fresh, &stale]), refs(&[&fresh, &offline])] {
        assert_eq!(
            evaluate(&catalog, &empty, &empty, &request, None)
                .unwrap_err()
                .code,
            ErrorCode::PolicyConflict
        );
    }
    let request = refs(&[&offline]);
    assert_eq!(
        resolve::evaluate(
            &catalog,
            Inputs {
                source: &empty,
                application: &empty,
                request: &request,
                pinned: None,
                operation: Operation::Compose,
                managed_output: true,
                update_due: true
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::PolicyConflict
    );
    let request = refs(&[&revision]);
    for operation in [Operation::Retrieve, Operation::Acquire] {
        assert!(
            resolve::evaluate(
                &catalog,
                Inputs {
                    source: &empty,
                    application: &empty,
                    request: &request,
                    pinned: None,
                    operation,
                    managed_output: false,
                    update_due: false
                }
            )
            .is_err()
        );
    }
}

#[test]
fn lookup_returns_full_definition_and_portable_catalog_cannot_shadow() {
    let policy = record(json!({"type":"revision_policy","history":"latest","keep":3}));
    let catalog = PolicyCatalog::new([policy.clone()]).unwrap();
    assert_eq!(catalog.get(policy.uuid).unwrap(), &policy);
    assert_eq!(
        catalog.get(Uuid::new_v4()).unwrap_err().code,
        ErrorCode::InvalidRequest
    );
    assert!(catalog.with_portable(&catalog).is_err());
}

#[test]
fn guide_examples_and_independent_verification_targets_are_valid() {
    let guide = include_str!("../docs/policy-resources.md");
    let block = guide
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let records: Vec<PolicyRecord> = serde_json::from_str(block).unwrap();
    PolicyCatalog::new(records).unwrap();
    let payload =
        record(json!({"type":"verification_policy","policy_target":"payload","mode":"none"}));
    let path = record(json!({"type":"verification_policy","policy_target":"path","mode":"full"}));
    let first = record(
        json!({"type":"github_release_asset_filter","platform":null,"asset_patterns":["a.zip"]}),
    );
    let second = record(
        json!({"type":"github_release_asset_filter","platform":null,"asset_patterns":["*.zip"]}),
    );
    let catalog =
        PolicyCatalog::new([payload.clone(), path.clone(), first.clone(), second.clone()]).unwrap();
    let empty = PolicyRefs::default();
    let app = refs(&[&payload, &path]);
    let plan = resolve::evaluate(
        &catalog,
        Inputs {
            source: &empty,
            application: &app,
            request: &empty,
            pinned: None,
            operation: Operation::Configure,
            managed_output: true,
            update_due: false,
        },
    )
    .unwrap();
    assert_eq!(
        plan.behavior.payload_verification,
        saucepan::models::policy::PayloadVerification::None
    );
    assert_eq!(
        plan.behavior.path_verification,
        saucepan::models::store::VerificationMode::Full
    );
    let ordered = resolve::filters(&catalog, &refs(&[&second, &first])).unwrap();
    assert_eq!(ordered[0].asset_patterns, ["*.zip"]);
    assert!(resolve::filters(&catalog, &refs(&[&path])).is_err());
}

#[test]
fn unmanaged_operations_project_managed_defaults_but_reject_explicit_retention() {
    let revision = record(json!({"type":"revision_policy","history":"latest","keep":3}));
    let catalog = PolicyCatalog::new([revision.clone()]).unwrap();
    let empty = PolicyRefs::default();
    let app = refs(&[&revision]);
    resolve::evaluate(
        &catalog,
        Inputs {
            source: &empty,
            application: &app,
            request: &empty,
            pinned: None,
            operation: Operation::Acquire,
            managed_output: false,
            update_due: false,
        },
    )
    .unwrap();
    assert!(
        resolve::evaluate(
            &catalog,
            Inputs {
                source: &empty,
                application: &empty,
                request: &app,
                pinned: None,
                operation: Operation::Acquire,
                managed_output: false,
                update_due: false
            }
        )
        .is_err()
    );
}
