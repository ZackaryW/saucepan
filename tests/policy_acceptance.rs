use saucepan::{
    core::{
        Store, claim::session::ApplicationRegistration, policy::catalog::PolicyCatalog,
        retrieval::Retriever, store::Acquirer,
    },
    models::{
        claim::EnsureRequest,
        policy::{
            PolicyRecord, PolicyRefs,
            requests::{Acquisition, Retrieval},
        },
        retrieval::SourceInput,
        source::Source,
        store::{Availability, Destination, Selector, VerificationMode},
    },
};
use serde_json::json;
use std::{fs, path::Path, process::Command};
use uuid::Uuid;

fn policy(mut value: serde_json::Value) -> PolicyRecord {
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
fn git(repo: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn commit(repo: &Path, file: &str, bytes: &str) {
    fs::write(repo.join(file), bytes).unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", bytes]);
}

#[test]
fn root_free_git_worker() {
    let Some(origin) = std::env::var_os("SAUCEPAN_ACCEPTANCE_ORIGIN") else {
        return;
    };
    let root = std::env::current_dir().unwrap();
    let no_store = policy(json!({"type":"cache_policy","mode":"no_store"}));
    let request = Retrieval {
        source: SourceInput::Inline {
            source: Source::Git {
                origin: origin.to_string_lossy().into(),
            },
        },
        selector: Selector::Ref {
            reference: "main".into(),
        },
        asset: Default::default(),
        policy_refs: refs(&[&no_store]),
        destination: Some(root.join("source-copy")),
        expected_sha256: None,
    };
    let catalog = PolicyCatalog::new([no_store]).unwrap();
    let retrieved = Retriever::retrieve(&request, &catalog).unwrap();
    assert!(retrieved.source_path().exists());
    let output = Acquirer::acquire_referenced(
        &retrieved,
        &Acquisition {
            destination: Destination::Directory {
                path: root.join("copied"),
            },
            folder: Some("packages/tool".into()),
            ..Default::default()
        },
        &catalog,
        None,
    )
    .unwrap();
    assert_eq!(fs::read(output.path().join("resource")).unwrap(), b"third");
    drop((output, retrieved));
    assert_eq!(fs::read(root.join("copied/resource")).unwrap(), b"third");
    for forbidden in [
        ".saucepan",
        "index",
        "policies",
        "claims",
        "store",
        "root.json",
    ] {
        assert!(
            !root.join(forbidden).exists(),
            "unexpected managed state: {forbidden}"
        );
    }
}

#[test]
fn two_scope_git_consumer_adopts_updates_reclaims_and_fetches_without_a_store() {
    let temp = tempfile::tempdir().unwrap();
    let origin = temp.path().join("origin");
    fs::create_dir_all(origin.join("packages/tool")).unwrap();
    git(&origin, &["init", "-b", "main"]);
    commit(&origin, "packages/tool/resource", "first");
    let store = Store::create(&temp.path().join("saucepan")).unwrap();
    let source = store
        .register_source(Source::Git {
            origin: origin.to_string_lossy().into(),
        })
        .unwrap();
    let access = policy(
        json!({"type":"access_policy","sources":[source.uuid],"scopes":["project-a","project-b"]}),
    );
    let revision = policy(json!({"type":"revision_policy","history":"latest","keep":2}));
    let reclaim = policy(json!({"type":"reclaim_policy","strategy":"restore_exact"}));
    store
        .register_policies(&[access.clone(), revision.clone(), reclaim.clone()])
        .unwrap();
    let app = store
        .register_application(
            &ApplicationRegistration {
                registration_key: "power-eagle-fixture".to_string().try_into().unwrap(),
                name: "Power Eagle-shaped consumer".into(),
                credentials_required: false,
                policy_refs: refs(&[&access]),
            },
            None,
        )
        .unwrap()
        .application
        .uuid;
    let session = store.session(app, None).unwrap();
    let mut first_request = EnsureRequest {
        scope: "project-a".to_string().try_into().unwrap(),
        source_uuid: source.uuid,
        selector: Selector::Ref {
            reference: "main".into(),
        },
        selection: Default::default(),
        context: [("package".into(), "tool".into())].into(),
        policy_refs: refs(&[&revision, &reclaim]),
    };
    first_request.selection.folder = Some("packages/tool".into());
    let first = session.ensure(&first_request, &Default::default()).unwrap();
    let mut second_request = first_request.clone();
    second_request.scope = "project-b".to_string().try_into().unwrap();
    let second = session
        .ensure(&second_request, &Default::default())
        .unwrap();
    assert_ne!(first.claim.uuid, second.claim.uuid);
    assert_eq!(first.claim.entry_uuid, second.claim.entry_uuid);
    assert_eq!(
        session
            .ensure(&first_request, &Default::default())
            .unwrap()
            .claim
            .uuid,
        first.claim.uuid
    );
    assert_eq!(session.claims().unwrap().len(), 2);
    let saved_b = serde_json::to_value(&second.claim).unwrap();
    let historical = fs::read(
        store
            .root()
            .join(format!("index/{}/index.json", first.claim.entry_uuid)),
    )
    .unwrap();
    let guard = session
        .lease_claim(second.claim.uuid, VerificationMode::Full)
        .unwrap();

    // Immutable registration does not adopt; one explicit scope adopts the new UUID.
    let automatic = policy(json!({"type":"update_policy","mode":"always"}));
    store
        .register_policies(std::slice::from_ref(&automatic))
        .unwrap();
    assert_eq!(
        store.registry(app).unwrap().claims[&first.claim.uuid].policy_refs,
        first.claim.policy_refs
    );
    let adopted_refs = refs(&[&revision, &reclaim, &automatic]);
    session
        .adopt_claim_policies(
            first.claim.uuid,
            store.application(app).unwrap().revision,
            adopted_refs.clone(),
        )
        .unwrap();
    first_request.policy_refs = adopted_refs;
    assert_eq!(
        serde_json::to_value(&store.registry(app).unwrap().claims[&second.claim.uuid]).unwrap(),
        saved_b
    );
    assert_eq!(
        fs::read(
            store
                .root()
                .join(format!("index/{}/index.json", first.claim.entry_uuid))
        )
        .unwrap(),
        historical
    );

    commit(&origin, "unselected", "outside");
    let unchanged = session.ensure(&first_request, &Default::default()).unwrap();
    assert_eq!(unchanged.claim.entry_uuid, first.claim.entry_uuid);
    commit(&origin, "packages/tool/resource", "second");
    let updated = session.ensure(&first_request, &Default::default()).unwrap();
    assert_ne!(updated.claim.entry_uuid, first.claim.entry_uuid);
    assert_eq!(
        fs::read(updated.acquired.directory.join("resource")).unwrap(),
        b"second"
    );
    let b = session
        .ensure(&second_request, &Default::default())
        .unwrap();
    assert_eq!(serde_json::to_value(&b.claim).unwrap(), saved_b);
    assert_eq!(
        fs::read(b.acquired.directory.join("resource")).unwrap(),
        b"first"
    );
    commit(&origin, "packages/tool/resource", "third");
    session.ensure(&first_request, &Default::default()).unwrap();
    assert!(
        store
            .reclaim()
            .unwrap()
            .deferred
            .contains_key(&first.claim.entry_uuid)
    );
    // Copy under a live native guard; the caller-owned copy has an independent lifetime.
    let owned = temp.path().join("project-copy");
    fs::create_dir(&owned).unwrap();
    fs::copy(guard.directory().join("resource"), owned.join("resource")).unwrap();
    drop(guard);
    assert!(
        store
            .reclaim()
            .unwrap()
            .reclaimed
            .contains(&first.claim.entry_uuid)
    );
    assert_eq!(
        store.output(first.claim.entry_uuid).unwrap().state,
        Availability::Reclaimed
    );
    let restored = session
        .recover(second.claim.uuid, &Default::default())
        .unwrap();
    assert_eq!(restored.claim.uuid, second.claim.uuid);
    assert_eq!(
        restored.acquired.entry.resolved,
        first.acquired.entry.resolved
    );
    assert_eq!(
        fs::read(restored.acquired.directory.join("resource")).unwrap(),
        b"first"
    );
    assert_eq!(fs::read(owned.join("resource")).unwrap(), b"first");
    session.release(first.claim.uuid).unwrap();
    session.release(second.claim.uuid).unwrap();
    assert!(session.claims().unwrap().is_empty());
    assert_eq!(store.policy(automatic.uuid).unwrap(), automatic);

    let before = saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap();
    let worker = temp.path().join("root-free");
    fs::create_dir(&worker).unwrap();
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "root_free_git_worker", "--nocapture"])
        .env("SAUCEPAN_ACCEPTANCE_ORIGIN", &origin)
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .current_dir(&worker)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        before,
        saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap()
    );
}
