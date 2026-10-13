mod support;
use saucepan::{
    core::{
        Store,
        policy::catalog::PolicyCatalog,
        retrieval::{RegisteredRetriever, RetrieveOptions, Retriever},
        store::Acquirer,
    },
    error::{Error, ErrorCode},
    models::{
        policy::{
            PolicyRecord, PolicyRefs,
            requests::{Acquisition, Retrieval},
        },
        retrieval::{Ownership, SourceInput},
        source::Source,
        store::{Destination, Selector},
    },
};
use serde_json::json;
use std::{fs, path::Path, process::Command};
use uuid::Uuid;

fn record(mut value: serde_json::Value) -> PolicyRecord {
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
fn input(source: SourceInput) -> Retrieval {
    Retrieval {
        source,
        selector: Selector::Local,
        asset: Default::default(),
        policy_refs: Default::default(),
        destination: None,
        expected_sha256: None,
    }
}
fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}

#[test]
fn scoped_caller_output_ignores_managed_defaults_but_keeps_update_constraints() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, "content").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let temporary = record(json!({"type":"cache_policy","mode":"temporary"}));
    let protect = record(json!({"type":"useable_app_policy"}));
    let retain = record(json!({"type":"revision_policy","history":"latest","keep":2}));
    let bounds = record(
        json!({"type":"policy_constraints","minimum_keep":2,"maximum_keep":3,"require_useable_app":true,"allow_automatic_updates":false}),
    );
    let update = record(json!({"type":"update_policy","mode":"always"}));
    store
        .register_policies(&[
            temporary.clone(),
            protect.clone(),
            retain.clone(),
            bounds.clone(),
            update.clone(),
        ])
        .unwrap();
    store
        .adopt_application_policies(
            app.uuid,
            app.revision,
            refs(&[&protect, &retain, &bounds]),
            None,
        )
        .unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let mut retrieval = input(SourceInput::Registered {
        source_uuid: source.uuid,
    });
    retrieval.policy_refs = refs(&[&temporary]);
    let mut acquisition = Acquisition {
        destination: Destination::Temporary {},
        ..Default::default()
    };
    let before = tree(store.root());
    let result = session
        .compose_referenced(
            &retrieval,
            &acquisition,
            &PolicyCatalog::default(),
            None,
            &Default::default(),
        )
        .unwrap();
    assert_eq!(
        fs::read_to_string(result.path().join("input")).unwrap(),
        "content"
    );
    assert_eq!(tree(store.root()), before);
    acquisition.policy_refs = refs(&[&update]);
    assert_eq!(
        code(
            session
                .compose_referenced(
                    &retrieval,
                    &acquisition,
                    &PolicyCatalog::default(),
                    None,
                    &Default::default()
                )
                .err()
                .unwrap()
        ),
        ErrorCode::PolicyConflict
    );
    assert_eq!(tree(store.root()), before);
}

#[test]
fn flat_guide_examples_are_valid_operation_inputs() {
    let guide = include_str!("../docs/composition.md");
    let examples: Vec<_> = guide
        .split("```json\n")
        .skip(1)
        .take(3)
        .map(|part| part.split("```").next().unwrap())
        .collect();
    let definitions: Vec<PolicyRecord> = serde_json::from_str(examples[0]).unwrap();
    let catalog = PolicyCatalog::new(definitions).unwrap();
    let retrieval: Retrieval = serde_json::from_str(examples[1]).unwrap();
    let acquisition: Acquisition = serde_json::from_str(examples[2]).unwrap();
    let empty = PolicyRefs::default();
    let attachments = saucepan::core::policy::prepare::Attachments {
        source: &empty,
        application: &empty,
    };
    saucepan::core::policy::prepare::retrieval(&catalog, &attachments, &retrieval).unwrap();
    saucepan::core::policy::prepare::acquisition(&catalog, &attachments, &acquisition).unwrap();
}
fn tree(path: &Path) -> saucepan::utils::tree::TreeSummary {
    saucepan::utils::tree::inspect(path, Default::default()).unwrap()
}

#[test]
fn root_free_worker() {
    let Some(path) = std::env::var_os("SAUCEPAN_POLICY_INPUT") else {
        return;
    };
    let temporary = record(json!({"type":"cache_policy", "mode":"temporary"}));
    let no_store = record(json!({"type":"cache_policy", "mode":"no_store"}));
    let catalog = PolicyCatalog::new([temporary.clone(), no_store.clone()]).unwrap();
    let mut request = input(SourceInput::Inline {
        source: Source::Local {
            path: path.clone().into(),
        },
    });
    request.policy_refs = refs(&[&temporary]);
    let retrieved =
        Retriever::retrieve_referenced(&request, &catalog, &Default::default()).unwrap();
    let leased = retrieved.source_path().to_owned();
    assert_eq!(fs::read(&leased).unwrap(), b"fresh");
    let copied = Acquirer::acquire_referenced(
        &retrieved,
        &Acquisition {
            destination: Destination::Temporary {},
            ..Default::default()
        },
        &catalog,
        None,
    )
    .unwrap();
    assert!(copied.path().exists());
    drop(retrieved);
    assert!(!leased.exists());
    assert!(copied.path().exists());
    request.policy_refs = refs(&[&no_store]);
    request.destination = Some(Path::new(&path).with_file_name("owned"));
    drop(Retriever::retrieve_referenced(&request, &catalog, &Default::default()).unwrap());
    assert_eq!(
        fs::read(request.destination.as_ref().unwrap()).unwrap(),
        b"fresh"
    );
    assert!(Retriever::retrieve_referenced(&request, &catalog, &Default::default()).is_err());
}

#[test]
fn portable_catalog_is_root_free_and_native_handles_keep_distinct_lifetimes() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    fs::write(&input, "fresh").unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "root_free_worker", "--nocapture"])
        .env("SAUCEPAN_POLICY_INPUT", &input)
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
}

#[test]
fn registered_fresh_is_observational_and_durable_portable_policies_fail_before_effects() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("input");
    fs::write(&path, "fresh").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path }).unwrap();
    let mut request = input(SourceInput::Registered {
        source_uuid: source.uuid,
    });
    let temporary = record(json!({"type":"cache_policy", "mode":"temporary"}));
    let refresh = record(json!({"type":"refresh_policy", "mode":"always"}));
    let catalog = PolicyCatalog::new([temporary.clone(), refresh.clone()]).unwrap();
    let before = tree(store.root());
    request.policy_refs = refs(&[&refresh]);
    let error = store
        .retrieve_referenced(&request, &catalog, &Default::default())
        .err()
        .unwrap();
    assert_eq!(code(error), ErrorCode::InvalidRequest);
    assert_eq!(tree(store.root()), before);
    request.policy_refs = refs(&[&temporary]);
    let reader = RegisteredRetriever::open(store.root()).unwrap();
    let retrieved = reader
        .retrieve_referenced(&request, &catalog, &Default::default())
        .unwrap();
    assert_eq!(fs::read(retrieved.source_path()).unwrap(), b"fresh");
    drop(retrieved);
    assert_eq!(tree(store.root()), before);
    // Authority cannot arrive from a caller-owned catalog.
    let grant = record(json!({"type":"access_policy"}));
    request.policy_refs = refs(&[&grant]);
    let catalog = PolicyCatalog::new([grant]).unwrap();
    assert_eq!(
        code(
            reader
                .retrieve_referenced(&request, &catalog, &Default::default())
                .err()
                .unwrap()
        ),
        ErrorCode::InvalidRequest
    );
    assert_eq!(tree(store.root()), before);
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn referenced_git_updates_compare_the_selected_subpath_and_retain_definition_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let origin = temp.path().join("origin");
    fs::create_dir_all(origin.join("selected")).unwrap();
    fs::write(origin.join("selected/file"), "first").unwrap();
    git(&origin, &["init", "-b", "main"]);
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "first"]);
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Git {
            origin: origin.to_str().unwrap().into(),
        })
        .unwrap();
    let update = record(json!({"type":"update_policy", "mode":"always"}));
    let history = record(json!({"type":"revision_policy", "history":"none"}));
    store
        .register_policies(&[update.clone(), history.clone()])
        .unwrap();
    let mut request = input(SourceInput::Registered {
        source_uuid: source.uuid,
    });
    request.selector = Selector::Ref {
        reference: "main".into(),
    };
    let acquisition = Acquisition {
        folder: Some("selected".into()),
        policy_refs: refs(&[&update, &history]),
        ..Default::default()
    };
    let options = RetrieveOptions::default();
    let catalog = PolicyCatalog::default();
    let first = store
        .compose_referenced(&request, &acquisition, &catalog, &options)
        .unwrap();
    let old = first.path().to_owned();
    fs::write(origin.join("unrelated"), "new").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "unrelated"]);
    let same = store
        .compose_referenced(&request, &acquisition, &catalog, &options)
        .unwrap();
    assert!(same.descriptor().reused);
    assert_eq!(same.path(), first.path());
    drop(same);
    fs::write(origin.join("selected/file"), "second").unwrap();
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "selected"]);
    let changed = store
        .compose_referenced(&request, &acquisition, &catalog, &options)
        .unwrap();
    assert_ne!(changed.path(), first.path());
    assert!(matches!(
        changed.descriptor().ownership,
        Ownership::Managed { .. }
    ));
    assert_eq!(fs::read(changed.path().join("file")).unwrap(), b"second");
    assert!(
        old.exists(),
        "history-none must preserve a live output lease"
    );
    assert!(store.records().unwrap().iter().any(|record| {
        match record {
            saucepan::models::index::Record::Acquisition(state) => state
                .acquisition
                .provenance
                .iter()
                .any(|e| e.uuid == update.uuid),
            _ => false,
        }
    }));
}

#[test]
fn release_filters_keep_explicit_order_and_share_discovery_without_a_checkout() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let discoveries = Arc::new(AtomicUsize::new(0));
    let count = discoveries.clone();
    let server = support::Server::new(move |path, base| {
        if path.starts_with("/repos/") {
            let n = count.fetch_add(1, Ordering::SeqCst);
            let mut assets = vec![
                json!({"id":1,"name":"first.bin","browser_download_url":format!("{base}/first")}),
                json!({"id":2,"name":"preferred.bin","browser_download_url":format!("{base}/preferred")}),
            ];
            if n.is_multiple_of(2) {
                assets.reverse();
            }
            (
                200,
                json!({"id":9,"tag_name":"v1","draft":false,"prerelease":false,"assets":assets})
                    .to_string()
                    .into_bytes(),
            )
        } else {
            (200, path.as_bytes().to_vec())
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/repo".into(),
        })
        .unwrap();
    let preferred =
        record(json!({"type":"github_release_asset_filter","asset_patterns":["preferred.bin"]}));
    let first =
        record(json!({"type":"github_release_asset_filter","asset_patterns":["first.bin"]}));
    let always = record(json!({"type":"refresh_policy","mode":"always"}));
    store
        .register_policies(&[preferred.clone(), first.clone(), always.clone()])
        .unwrap();
    let mut request = input(SourceInput::Registered {
        source_uuid: source.uuid,
    });
    request.selector = Selector::LatestStable;
    request.asset.filter_refs = refs(&[&preferred, &first]);
    let options = RetrieveOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
    };
    let catalog = PolicyCatalog::default();
    for refresh in [false, false, true] {
        request.policy_refs = if refresh {
            refs(&[&always])
        } else {
            Default::default()
        };
        let retrieved = store
            .retrieve_referenced(&request, &catalog, &options)
            .unwrap();
        assert_eq!(fs::read(retrieved.source_path()).unwrap(), b"/preferred");
        assert!(
            retrieved
                .request()
                .provenance
                .iter()
                .any(|e| e.uuid == preferred.uuid)
        );
    }
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
    assert!(
        !store
            .root()
            .join(format!("source/{}/repo.git", source.uuid))
            .exists()
    );
    request.asset.filter_refs = refs(&[&always]);
    let before = tree(store.root());
    assert_eq!(
        code(
            store
                .retrieve_referenced(&request, &catalog, &options)
                .err()
                .unwrap()
        ),
        ErrorCode::InvalidRequest
    );
    assert_eq!(discoveries.load(Ordering::SeqCst), 2);
    assert_eq!(before, tree(store.root()));
}

#[test]
fn update_permission_is_checked_when_due_without_rejecting_the_initial_acquisition() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("input");
    fs::write(&file, "first").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store.register_source(Source::Local { path: file }).unwrap();
    let access = record(json!({"type":"access_policy","actions":["acquire"]}));
    let update = record(json!({"type":"update_policy","mode":"always"}));
    store
        .register_policies(&[access.clone(), update.clone()])
        .unwrap();
    store
        .adopt_source_policies(source.uuid, 0, refs(&[&access]))
        .unwrap();
    let request = input(SourceInput::Registered {
        source_uuid: source.uuid,
    });
    let acquisition = Acquisition {
        policy_refs: refs(&[&update]),
        ..Default::default()
    };
    let first = store
        .compose_referenced(
            &request,
            &acquisition,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
    let before = tree(store.root());
    assert_eq!(
        code(
            store
                .compose_referenced(
                    &request,
                    &acquisition,
                    &Default::default(),
                    &Default::default()
                )
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    assert_eq!(tree(store.root()), before);
    assert!(first.path().exists());
}
