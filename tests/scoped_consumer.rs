mod native_root;
mod policy_fixture;
use saucepan::{
    core::{Store, claim::session::ApplicationRegistration, query::ClaimQuery},
    error::{Error, ErrorCode},
    models::{
        claim::{CoveredView, EnsureRequest},
        policy::Identical,
        source::Source,
        store::{Availability, Selector, VerificationMode},
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn wait(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "consumer never ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn external_reclaim(root: &Path) {
    let result = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(root)
        .arg("reclaim")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn native_child() {
    let Some(base) = std::env::var_os("SAUCEPAN_NATIVE_CONSUMER") else {
        return;
    };
    let base = PathBuf::from(base);
    let store = Store::open(&base.join("root")).unwrap();
    let view: CoveredView =
        serde_json::from_slice(&fs::read(base.join("view.json")).unwrap()).unwrap();
    let session = store.session(view.app, None).unwrap();
    let work = || -> anyhow::Result<()> {
        let lease = session.lease_view(&view, VerificationMode::Full)?;
        fs::write(base.join("ready"), lease.entry_uuid().to_string())?;
        let start = Instant::now();
        loop {
            assert_eq!(fs::read(lease.directory().join("resource"))?, b"original");
            if base.join("release").exists() {
                break;
            }
            anyhow::ensure!(
                start.elapsed() < Duration::from_secs(20),
                "consumer timeout"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        anyhow::ensure!(
            std::env::var("SAUCEPAN_NATIVE_MODE")? != "error",
            "consumer read failed after lease acquisition"
        );
        Ok(())
    };
    let result = work(); // Native guard is dropped even when work returns an error.
    if let Err(error) = result {
        eprintln!("{}", Error::from_anyhow(&error));
        std::process::exit(3);
    }
}

#[test]
fn two_scopes_read_nested_packages_while_external_reclaim_waits_for_native_lifetimes() {
    for mode in ["normal", "error", "kill"] {
        let tmp = tempfile::tempdir().unwrap();
        let store = native_root::create(&tmp.path().join("root")).unwrap();
        let input = tmp.path().join("input");
        fs::create_dir_all(input.join("packages/tool")).unwrap();
        fs::write(input.join("packages/tool/resource"), "original").unwrap();
        fs::write(input.join("sibling"), "whole-output lease").unwrap();
        let source = store
            .register_source(Source::Local {
                path: input.clone(),
            })
            .unwrap();
        let registered = store
            .register_application(
                &ApplicationRegistration {
                    registration_key: "consumer".to_string().try_into().unwrap(),
                    name: "consumer".into(),
                    credentials_required: false,
                    policy_refs: policy_fixture::register(
                        &store,
                        Default::default(),
                        Default::default(),
                    )
                    .unwrap(),
                },
                None,
            )
            .unwrap();
        let app = registered.application.uuid;
        let session = store.session(app, None).unwrap();
        let mut request = EnsureRequest {
            scope: "project-a".to_string().try_into().unwrap(),
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            context: [("package".into(), "tool".into())].into(),
            policy_refs: Default::default(),
        };
        policy_fixture::replace(
            &store,
            &mut request.policy_refs,
            serde_json::json!({"type":"revision_policy","history":"latest","keep":1}),
        )
        .unwrap();
        let owner = session.ensure(&request, &Default::default()).unwrap();
        request.scope = "project-b".to_string().try_into().unwrap();
        let b = session.ensure(&request, &Default::default()).unwrap();
        assert_eq!(owner.claim.entry_uuid, b.claim.entry_uuid);
        request.scope = owner.claim.scope.clone();
        request.selection.folder = Some("packages/tool".into());
        let view = session.ensure(&request, &Default::default()).unwrap();
        assert_eq!(view.claim.uuid, owner.claim.uuid);
        fs::write(
            tmp.path().join("view.json"),
            serde_json::to_vec(&view.view).unwrap(),
        )
        .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "native_child", "--nocapture"])
            .env("SAUCEPAN_NATIVE_CONSUMER", tmp.path())
            .env("SAUCEPAN_NATIVE_MODE", mode)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        wait(&tmp.path().join("ready"));
        fs::write(input.join("packages/tool/resource"), "updated").unwrap();
        let updated = session
            .update(owner.claim.uuid, &Default::default())
            .unwrap();
        assert_ne!(updated.claim.entry_uuid, b.claim.entry_uuid);
        let found = session
            .query(&ClaimQuery {
                app,
                scope: Some(b.claim.scope.clone()),
                conditions: vec![],
            })
            .unwrap();
        assert_eq!(found.claims.len(), 1);
        assert_eq!(found.claims[0].claim.entry_uuid, b.claim.entry_uuid);
        external_reclaim(store.root());
        assert_eq!(
            store.output(b.claim.entry_uuid).unwrap().state,
            Availability::Ready
        );
        assert!(
            store
                .path(b.claim.entry_uuid)
                .unwrap()
                .join("sibling")
                .is_file()
        );
        session.release(owner.claim.uuid).unwrap();
        session.release(b.claim.uuid).unwrap();
        external_reclaim(store.root()); // Neither release nor absent claims revoke the existing guard.
        if mode == "kill" {
            child.kill().unwrap();
        } else {
            fs::write(tmp.path().join("release"), "release").unwrap();
        }
        let result = child.wait_with_output().unwrap();
        if mode == "normal" {
            assert!(result.status.success());
        } else {
            assert!(!result.status.success());
        }
        external_reclaim(store.root());
        assert_eq!(
            store.output(b.claim.entry_uuid).unwrap().state,
            Availability::Reclaimed
        );
    }
}

#[test]
fn failed_view_verification_drops_ownership_and_unavailable_claim_has_a_stable_code() {
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let input = tmp.path().join("resource");
    fs::write(&input, "original").unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap().uuid;
    let mut request = EnsureRequest {
        scope: "project".to_string().try_into().unwrap(),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: Default::default(),
        policy_refs: Default::default(),
    };
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"revision_policy","history":"latest","keep":1}),
    )
    .unwrap();
    let session = store.session(app, None).unwrap();
    let first = session.ensure(&request, &Default::default()).unwrap();
    fs::write(first.acquired.directory.join("resource"), "changed").unwrap();
    let error = session
        .lease_view(&first.view, VerificationMode::Full)
        .err()
        .unwrap();
    assert_eq!(Error::from_anyhow(&error).code, ErrorCode::IntegrityError);
    fs::write(first.acquired.directory.join("resource"), "original").unwrap();
    let mut pull = policy_fixture::intent(&first.claim.request);
    policy_fixture::replace(
        &store,
        &mut pull.policy_refs,
        serde_json::json!({"type":"identical_pull_policy","strategy":Identical::NewEntry}),
    )
    .unwrap();
    store.acquire(&pull).unwrap();
    external_reclaim(store.root());
    assert_eq!(
        store.output(first.claim.entry_uuid).unwrap().state,
        Availability::Reclaimed
    );
    assert_eq!(
        Error::from_anyhow(
            &session
                .lease_view(&first.view, VerificationMode::Full)
                .err()
                .unwrap()
        )
        .code,
        ErrorCode::ContentUnavailable
    );
    assert_eq!(session.claims().unwrap().len(), 1);
}
