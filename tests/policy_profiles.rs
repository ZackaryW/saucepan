use saucepan::{
    core::{
        Store,
        policy::{
            catalog::PolicyCatalog,
            prepare::{self, Attachments},
            profile,
        },
    },
    models::{
        policy::{
            Freshness, PolicyRecord, PolicyRefs, RevisionPolicy, UpdatePolicy,
            profile::{ProfileName, ProfileRequest},
            requests::Retrieval,
        },
        retrieval::SourceInput,
        source::Source,
        store::Selector,
    },
};
use serde_json::{Value, json};
use std::{fs, process::Command};
use uuid::Uuid;

#[test]
fn immutable_recipes_have_stage_specific_behavior() {
    let managed = profile::expand(&ProfileRequest::default()).unwrap();
    assert_eq!(
        managed,
        profile::expand(&ProfileRequest::default()).unwrap()
    );
    assert_eq!(managed.catalog.len(), 7);
    let expected = json!([
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0001","type":"cache_policy","mode":"cached"},
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0003","type":"refresh_policy","mode":"max_age","seconds":86400},
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0005","type":"stale_policy","allow_stale":false},
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0006","type":"update_policy","mode":"manual"},
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0007","type":"identical_pull_policy","strategy":"reuse"},
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0008","type":"revision_policy","history":"latest","keep":10},
        {"uuid":"acf53062-66cd-4dd9-94bb-c9f849eb0009","type":"reclaim_policy","strategy":"report_unavailable"}
    ]);
    assert_eq!(serde_json::to_value(&managed.catalog).unwrap(), expected);
    for name in profile::NAMES {
        let expanded = profile::expand(&ProfileRequest {
            name,
            ..Default::default()
        })
        .unwrap();
        let catalog = PolicyCatalog::new(expanded.catalog).unwrap();
        let empty = PolicyRefs::default();
        let request = Retrieval {
            source: SourceInput::Inline {
                source: Source::Local {
                    path: "absent".into(),
                },
            },
            selector: Selector::Local,
            asset: Default::default(),
            policy_refs: expanded.retrieval_policy_refs,
            destination: None,
            expected_sha256: None,
        };
        let prepared = prepare::retrieval(
            &catalog,
            &Attachments {
                source: &empty,
                application: &empty,
            },
            &request,
        )
        .unwrap();
        assert_eq!(
            prepared.policies.behavior.freshness,
            if name == ProfileName::Managed {
                Freshness::MaxAge { seconds: 86400 }
            } else {
                Freshness::Always {}
            }
        );
        if name == ProfileName::Temporary {
            assert!(expanded.acquisition_policy_refs.ids().is_empty());
        }
        assert_eq!(prepared.policies.behavior.update, UpdatePolicy::Manual {});
        assert_eq!(
            prepared.policies.behavior.revision,
            RevisionPolicy::Latest { keep: 10 }
        );
    }
}

#[test]
fn overrides_are_typed_and_identity_is_never_rewritten() {
    let record: PolicyRecord = serde_json::from_value(
        json!({"uuid":Uuid::new_v4(),"type":"revision_policy","history":"latest","keep":3}),
    )
    .unwrap();
    let mut request = ProfileRequest {
        catalog: vec![record.clone()],
        acquisition_policy_refs: vec![record.uuid].try_into().unwrap(),
        ..Default::default()
    };
    let result = profile::expand(&request).unwrap();
    assert!(result.catalog.contains(&record));
    assert!(
        !result
            .catalog
            .iter()
            .any(|r| r.uuid.to_string().ends_with("0008"))
    );
    let mut other = record.clone();
    other.uuid = Uuid::new_v4();
    request.catalog.push(other.clone());
    request.acquisition_policy_refs = vec![record.uuid, other.uuid].try_into().unwrap();
    assert_eq!(
        profile::expand(&request).unwrap_err().code,
        saucepan::error::ErrorCode::PolicyConflict
    );
    request.acquisition_policy_refs = Default::default();
    request.retrieval_policy_refs = vec![record.uuid].try_into().unwrap();
    assert_eq!(
        profile::expand(&request).unwrap_err().code,
        saucepan::error::ErrorCode::InvalidRequest
    );
    request.retrieval_policy_refs = vec![Uuid::new_v4()].try_into().unwrap();
    assert!(profile::expand(&request).is_err());
    request.retrieval_policy_refs = Default::default();
    request.catalog[0].uuid = "acf53062-66cd-4dd9-94bb-c9f849eb0008".parse().unwrap();
    assert_eq!(
        profile::expand(&request).unwrap_err().code,
        saucepan::error::ErrorCode::PolicyConflict
    );
}

#[test]
fn installation_preparation_is_explicit_and_rejects_fresh_storage() {
    assert!(
        profile::expand(&ProfileRequest {
            name: ProfileName::Temporary,
            for_install: true,
            ..Default::default()
        })
        .is_err()
    );
    let result = profile::expand(&ProfileRequest {
        for_install: true,
        ..Default::default()
    })
    .unwrap();
    assert!(result.catalog.iter().any(|r| matches!(
        r.definition,
        saucepan::models::policy::PolicyDefinition::UseableAppPolicy {}
    )));
    let record: PolicyRecord = serde_json::from_value(
        json!({"uuid":Uuid::new_v4(),"type":"cache_policy","mode":"no_store"}),
    )
    .unwrap();
    assert!(
        profile::expand(&ProfileRequest {
            for_install: true,
            retrieval_policy_refs: vec![record.uuid].try_into().unwrap(),
            catalog: vec![record],
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn cli_preview_is_root_free_and_registration_is_separate() {
    let tmp = tempfile::tempdir().unwrap();
    let absent = tmp.path().join("absent");
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .args(["--root", absent.to_str().unwrap(), "profile"])
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!absent.exists());
    let expansion: Value = serde_json::from_slice(&output.stdout).unwrap();
    let records: Vec<PolicyRecord> = serde_json::from_value(expansion["catalog"].clone()).unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let first = store.register_policies(&records).unwrap();
    assert_eq!(first, store.register_policies(&records).unwrap());
    let app = store.register_app("example".into()).unwrap();
    let catalog = tmp.path().join("catalog.json");
    fs::write(&catalog, serde_json::to_vec(&records).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .args([
            "--root",
            store.root().to_str().unwrap(),
            "--as-app",
            &app.uuid.to_string(),
            "register-policies",
            catalog.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn profile_consumer() {
    let Some(report) = std::env::var_os("PROFILE_REPORT") else {
        return;
    };
    let path = std::path::PathBuf::from(std::env::var_os("SAUCEPAN_SOURCE_PATH").unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "profile material");
    fs::write(report, path.to_str().unwrap()).unwrap();
}

#[test]
fn documented_managed_and_guarded_temporary_flows() {
    let tmp = tempfile::tempdir().unwrap();
    let original = tmp.path().join("original");
    fs::write(&original, "profile material").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local {
            path: original.clone(),
        })
        .unwrap();
    let managed = profile::expand(&ProfileRequest::default()).unwrap();
    store.register_policies(&managed.catalog).unwrap();
    let request = saucepan::models::store::Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policy_refs: managed.combined_policy_refs,
    };
    let a = store.acquire(&request).unwrap();
    let b = store.acquire(&request).unwrap();
    assert_eq!(a.entry.uuid, b.entry.uuid);
    let temporary = profile::expand(&ProfileRequest {
        name: ProfileName::Temporary,
        ..Default::default()
    })
    .unwrap();
    let catalog = tmp.path().join("portable.json");
    fs::write(&catalog, serde_json::to_vec(&temporary.catalog).unwrap()).unwrap();
    let input = tmp.path().join("retrieve.json");
    fs::write(&input, serde_json::to_vec(&json!({"source":{"kind":"inline","source":{"type":"local","path":original}},"selector":{"type":"local"},"policy_refs":temporary.retrieval_policy_refs})).unwrap()).unwrap();
    let report = tmp.path().join("report");
    let missing = tmp.path().join("must-not-exist");
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .args([
            "--root",
            missing.to_str().unwrap(),
            "--policies",
            catalog.to_str().unwrap(),
            "run-retrieved",
            input.to_str().unwrap(),
            "--",
        ])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "profile_consumer", "--nocapture"])
        .env("PROFILE_REPORT", &report)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!missing.exists());
    let scratch = fs::read_to_string(report).unwrap();
    assert!(!std::path::Path::new(&scratch).exists());
    assert!(original.exists());
}
