use saucepan::{
    core::{Store, activation::Probe},
    models::{
        index::lifecycle::{Installation, Phase},
        policy::{Identical, Protection},
        source::Source,
        store::{Request, Selector},
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn fixture_probe() {
    println!("LIFECYCLE-HEALTHY");
}

#[test]
fn fixture_blocked_probe() {
    let exe = std::env::current_exe().unwrap();
    let root = exe.parent().unwrap().parent().unwrap().parent().unwrap();
    if root.join("block-enabled").exists() {
        fs::write(root.join("probe-ready"), b"ready").unwrap();
        until(&root.join("probe-release"));
    }
    println!("LIFECYCLE-HEALTHY");
}
fn until(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn setup() -> (tempfile::TempDir, Store, Installation) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_str().unwrap().to_owned();
    let source = store.register_source(Source::Local { path: exe }).unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin = vec![name];
    request.policies.protection = Some(Protection::UseableApp);
    request.policies.identical = Identical::NewEntry;
    let input = Installation {
        name: "fixture".into(),
        request,
        probe: Probe {
            args: vec![
                "--exact".into(),
                "fixture_probe".into(),
                "--nocapture".into(),
            ],
            stdout_contains: "LIFECYCLE-HEALTHY".into(),
            timeout_seconds: 10,
        },
        shim: if cfg!(windows) {
            "fixture.exe"
        } else {
            "fixture"
        }
        .into(),
        saucepan: false,
    };
    (tmp, store, input)
}
fn shim() -> PathBuf {
    env!("CARGO_BIN_EXE_saucepan-shim").into()
}

#[test]
fn managed_claim_and_binding_commit_together_and_rollback_preserves_request() {
    let (_tmp, store, input) = setup();
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    assert_eq!(installed.status, Phase::Complete, "{:?}", installed.pending);
    let first = installed.candidate.unwrap();
    let initial = store.application(installed.app).unwrap();
    assert!(initial.claims[&installed.claim].context.is_empty());
    let context = [("project".into(), "lifecycle".into())].into();
    store
        .set_claim_context(installed.app, installed.claim, &context, initial.revision)
        .unwrap();
    let before = serde_json::to_value(store.application(installed.app).unwrap()).unwrap();
    let bad = Probe {
        stdout_contains: "never matched".into(),
        ..input.probe.clone()
    };
    let failed = store
        .update_application(
            installed.app,
            installed.claim,
            &bad,
            None,
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(failed.status, Phase::Failed);
    assert_eq!(
        serde_json::to_value(store.application(installed.app).unwrap()).unwrap(),
        before
    );
    let updated = store
        .update_application(
            installed.app,
            installed.claim,
            &input.probe,
            None,
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(updated.status, Phase::Complete);
    let app = store.application(installed.app).unwrap();
    assert_eq!(app.claims[&installed.claim].context, context);
    assert_eq!(
        app.claims[&installed.claim].entry_uuid,
        updated.candidate.unwrap()
    );
    assert_eq!(app.active.unwrap().entry_uuid, updated.candidate.unwrap());
    let rolled = store
        .update_application(
            installed.app,
            installed.claim,
            &input.probe,
            Some(first),
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(rolled.status, Phase::Complete);
    assert_eq!(
        store.application(installed.app).unwrap().claims[&installed.claim].context,
        context
    );
    assert_eq!(
        store.application(installed.app).unwrap().claims[&installed.claim].entry_uuid,
        first
    );
    assert_eq!(
        serde_json::to_value(
            &store.application(installed.app).unwrap().claims[&installed.claim].request
        )
        .unwrap(),
        serde_json::to_value(&input.request).unwrap()
    );
    let output = Command::new(store.root().join("bin").join(&input.shim))
        .args(&input.probe.args)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("LIFECYCLE-HEALTHY"));
    let replay = store
        .install_application(
            &input,
            &shim(),
            &Default::default(),
            Some(installed.operation),
        )
        .unwrap();
    assert_eq!(replay.app, installed.app);
}

#[test]
fn failed_install_has_no_live_application_and_cli_install_matches_native_results() {
    let (tmp, store, mut input) = setup();
    input.probe.stdout_contains = "cannot succeed".into();
    let failed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    assert_eq!(failed.status, Phase::Failed);
    assert!(store.application(failed.app).is_err());
    assert!(!store.root().join("bin").join(&input.shim).exists());
    input.probe.stdout_contains = "LIFECYCLE-HEALTHY".into();
    let request = tmp.path().join("request.json");
    let probe = tmp.path().join("probe.json");
    fs::write(&request, serde_json::to_vec(&input.request).unwrap()).unwrap();
    fs::write(&probe, serde_json::to_vec(&input.probe).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .arg("install")
        .arg(&input.name)
        .arg(request)
        .arg(probe)
        .arg(shim())
        .arg("--shim")
        .arg(&input.shim)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let installed: saucepan::models::index::lifecycle::Operation =
        serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        store
            .application(installed.app)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        installed.candidate.unwrap()
    );
}

#[test]
fn interrupted_preparation_rechecks_probe_without_publishing_a_transition() {
    use saucepan::models::index::lifecycle::Action;
    let (_tmp, store, input) = setup();
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    let before = serde_json::to_value(store.application(installed.app).unwrap()).unwrap();
    let probe = Probe {
        stdout_contains: "probe must be checked again".into(),
        ..input.probe.clone()
    };
    let id = uuid::Uuid::new_v4();
    let (_, guard) = store
        .begin_operation(
            id,
            Some(installed.app),
            Action::Update {
                claim: installed.claim,
                probe: probe.clone(),
                rollback: None,
            },
        )
        .unwrap();
    drop(guard);
    let reopened = Store::open(store.root()).unwrap();
    assert_eq!(
        reopened.reconcile(Some(id)).unwrap()[0].status,
        Phase::Preparing
    );
    let result = reopened
        .update_application(
            installed.app,
            installed.claim,
            &probe,
            None,
            &Default::default(),
            Some(id),
        )
        .unwrap();
    assert_eq!(result.operation, id);
    assert_eq!(result.status, Phase::Failed);
    assert_eq!(
        serde_json::to_value(reopened.application(installed.app).unwrap()).unwrap(),
        before
    );
}

#[test]
fn managed_update_detects_deactivation_during_probe() {
    let (_tmp, store, input) = setup();
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    fs::write(store.root().join("block-enabled"), b"enabled").unwrap();
    let probe = Probe {
        args: vec![
            "--exact".into(),
            "fixture_blocked_probe".into(),
            "--nocapture".into(),
        ],
        ..input.probe.clone()
    };
    std::thread::scope(|scope| {
        let update = scope.spawn(|| {
            store
                .update_application(
                    installed.app,
                    installed.claim,
                    &probe,
                    None,
                    &Default::default(),
                    None,
                )
                .unwrap()
        });
        until(&store.root().join("probe-ready"));
        store.deactivate(installed.app).unwrap();
        fs::write(store.root().join("probe-release"), b"release").unwrap();
        assert_eq!(update.join().unwrap().status, Phase::Conflict);
    });
    let app = store.application(installed.app).unwrap();
    assert!(app.active.is_none());
    assert_eq!(
        app.claims[&installed.claim].entry_uuid,
        installed.candidate.unwrap()
    );
}

#[test]
fn managed_transitions_detect_context_edits_during_probe() {
    for rollback in [false, true] {
        let (_tmp, store, input) = setup();
        let installed = store
            .install_application(&input, &shim(), &Default::default(), None)
            .unwrap();
        let before = store.application(installed.app).unwrap();
        fs::write(store.root().join("block-enabled"), b"enabled").unwrap();
        let probe = Probe {
            args: vec![
                "--exact".into(),
                "fixture_blocked_probe".into(),
                "--nocapture".into(),
            ],
            ..input.probe.clone()
        };
        let context = [("project".into(), "changed".into())].into();
        std::thread::scope(|scope| {
            let update = scope.spawn(|| {
                store
                    .update_application(
                        installed.app,
                        installed.claim,
                        &probe,
                        if rollback { installed.candidate } else { None },
                        &Default::default(),
                        None,
                    )
                    .unwrap()
            });
            until(&store.root().join("probe-ready"));
            store
                .set_claim_context(installed.app, installed.claim, &context, before.revision)
                .unwrap();
            fs::write(store.root().join("probe-release"), b"release").unwrap();
            assert_eq!(update.join().unwrap().status, Phase::Conflict);
        });
        let app = store.application(installed.app).unwrap();
        assert_eq!(app.claims[&installed.claim].context, context);
        assert_eq!(
            app.claims[&installed.claim].entry_uuid,
            installed.candidate.unwrap()
        );
        assert_eq!(
            app.active.unwrap().entry_uuid,
            before.active.unwrap().entry_uuid
        );
        store.deactivate(installed.app).unwrap();
        assert_eq!(
            store.application(installed.app).unwrap().claims[&installed.claim].context,
            context
        );
        store.uninstall(installed.app, false, None).unwrap();
        assert!(store.application(installed.app).is_err());
    }
}

#[test]
fn managed_update_rejects_aba_and_foreign_rollback() {
    use saucepan::models::index::{Settings, lifecycle::Action};
    let (_tmp, store, input) = setup();
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    let id = uuid::Uuid::new_v4();
    let (_, guard) = store
        .begin_operation(
            id,
            Some(installed.app),
            Action::Update {
                claim: installed.claim,
                probe: input.probe.clone(),
                rollback: None,
            },
        )
        .unwrap();
    drop(guard);
    store
        .set_settings(
            installed.app,
            Settings {
                source_filter: vec![input.request.source_uuid],
            },
        )
        .unwrap();
    store
        .set_settings(installed.app, Default::default())
        .unwrap();
    let result = store
        .update_application(
            installed.app,
            installed.claim,
            &input.probe,
            None,
            &Default::default(),
            Some(id),
        )
        .unwrap();
    assert_eq!(result.status, Phase::Conflict);
    let mut foreign = input.request.clone();
    foreign.selection.bin.clear();
    let other = store.acquire(&foreign).unwrap();
    let rejected = store
        .update_application(
            installed.app,
            installed.claim,
            &input.probe,
            Some(other.entry.uuid),
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(rejected.status, Phase::Failed);
    assert_eq!(
        store
            .application(installed.app)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        installed.candidate.unwrap()
    );
}

#[test]
fn uninstall_preserves_shared_outputs_and_live_target_then_reconciles() {
    use saucepan::models::{policy::Eviction, store::Availability};
    let (_tmp, store, mut input) = setup();
    input.request.policies.eviction = Eviction::Latest { keep: 1 };
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    let first = installed.candidate.unwrap();
    let other = store.register_app("shared".into()).unwrap();
    let mut shared = input.request.clone();
    shared.policies.identical = Identical::Reuse;
    let claim = store
        .acquire_for(other.uuid, &shared, &Default::default())
        .unwrap();
    assert_eq!(claim.claim.entry_uuid, first);
    store
        .activate(other.uuid, first, &shared.selection.bin[0], &input.probe)
        .unwrap();
    let second = store.acquire(&input.request).unwrap();
    fs::write(store.root().join("block-enabled"), b"enabled").unwrap();
    let mut target = Command::new(store.root().join("bin").join(&input.shim))
        .args(["--exact", "fixture_blocked_probe", "--nocapture"])
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    until(&store.root().join("probe-ready"));
    let result = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .arg("uninstall")
        .arg(installed.app.to_string())
        .output()
        .unwrap();
    let removed: saucepan::models::index::lifecycle::Operation =
        serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        removed.status,
        if cfg!(windows) {
            Phase::CleanupPending
        } else {
            Phase::Complete
        },
        "{:?}",
        removed.pending
    );
    assert!(store.application(installed.app).is_err());
    assert!(target.try_wait().unwrap().is_none());
    assert!(store.reclaim().unwrap().deferred.contains_key(&first));
    assert_eq!(
        store.uninstall(other.uuid, false, None).unwrap().status,
        Phase::Complete
    );
    assert!(store.reclaim().unwrap().deferred.contains_key(&first));
    fs::write(store.root().join("probe-release"), b"release").unwrap();
    assert!(target.wait().unwrap().success());
    let final_result = store.reconcile(Some(removed.operation)).unwrap().remove(0);
    assert_eq!(
        final_result.status,
        Phase::Complete,
        "{:?}",
        final_result.pending
    );
    assert!(
        !store
            .root()
            .join("index")
            .join(installed.app.to_string())
            .exists()
    );
    assert_eq!(
        store
            .uninstall(installed.app, false, None)
            .unwrap()
            .operation,
        removed.operation
    );
    assert!(store.uninstall(uuid::Uuid::new_v4(), false, None).is_err());
    store.reclaim().unwrap();
    assert_eq!(store.output(first).unwrap().state, Availability::Reclaimed);
    assert!(second.directory.exists());
    assert!(store.source(input.request.source_uuid).is_ok());
}

#[test]
fn uninstall_fences_both_managed_and_binding_only_inflight_probes() {
    for managed in [false, true] {
        let (_tmp, store, input) = setup();
        let installed = store
            .install_application(&input, &shim(), &Default::default(), None)
            .unwrap();
        fs::write(store.root().join("block-enabled"), b"enabled").unwrap();
        let probe = Probe {
            args: vec![
                "--exact".into(),
                "fixture_blocked_probe".into(),
                "--nocapture".into(),
            ],
            ..input.probe.clone()
        };
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                if managed {
                    store
                        .update_application(
                            installed.app,
                            installed.claim,
                            &probe,
                            None,
                            &Default::default(),
                            None,
                        )
                        .unwrap()
                        .status
                        != Phase::Complete
                } else {
                    store
                        .activate(
                            installed.app,
                            installed.candidate.unwrap(),
                            &input.request.selection.bin[0],
                            &probe,
                        )
                        .is_err()
                }
            });
            until(&store.root().join("probe-ready"));
            assert_eq!(
                store.uninstall(installed.app, false, None).unwrap().status,
                Phase::Complete
            );
            fs::write(store.root().join("probe-release"), b"release").unwrap();
            assert!(worker.join().unwrap());
        });
        assert!(store.application(installed.app).is_err());
    }
}

#[test]
fn uninstall_preserves_changed_and_unknown_files_without_blocking_other_apps() {
    let (_tmp, store, input) = setup();
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    let path = store.root().join("bin").join(&input.shim);
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"externally replaced").unwrap();
    let removed = store.uninstall(installed.app, false, None).unwrap();
    assert_eq!(removed.status, Phase::CleanupPending);
    assert_eq!(fs::read(&path).unwrap(), b"externally replaced");
    assert!(
        Store::open(store.root())
            .unwrap()
            .register_app("independent".into())
            .is_ok()
    );
    fs::write(&path, original).unwrap();
    let sidecar = path.with_extension("shim.json");
    let config = fs::read(&sidecar).unwrap();
    fs::write(&sidecar, b"changed after first removal attempt").unwrap();
    assert_eq!(
        store.reconcile(Some(removed.operation)).unwrap()[0].status,
        Phase::CleanupPending
    );
    assert_eq!(
        fs::read(&sidecar).unwrap(),
        b"changed after first removal attempt"
    );
    assert!(path.exists(), "pair preflight preserves the executable too");
    fs::write(&sidecar, config).unwrap();
    let unknown = store
        .root()
        .join("index")
        .join(installed.app.to_string())
        .join("notes.txt");
    fs::write(&unknown, b"preserve").unwrap();
    assert_eq!(
        store.reconcile(Some(removed.operation)).unwrap()[0].status,
        Phase::CleanupPending
    );
    assert_eq!(fs::read(&unknown).unwrap(), b"preserve");
    assert!(
        Store::open(store.root())
            .unwrap()
            .register_app("still independent".into())
            .is_ok()
    );
    fs::remove_file(unknown).unwrap();
    assert_eq!(
        store.reconcile(Some(removed.operation)).unwrap()[0].status,
        Phase::Complete
    );
}

#[test]
fn unmanaged_shims_require_explicit_adoption_or_exclusion() {
    let (_tmp, store, input) = setup();
    let installed = store
        .install_application(&input, &shim(), &Default::default(), None)
        .unwrap();
    let old_name = if cfg!(windows) {
        "unregistered.exe"
    } else {
        "unregistered"
    };
    let old = store.root().join("bin").join(old_name);
    fs::copy(shim(), &old).unwrap();
    fs::write(
        old.with_extension("shim.json"),
        serde_json::to_vec_pretty(&saucepan::core::launch::ShimConfig {
            app: installed.app,
            saucepan: false,
        })
        .unwrap(),
    )
    .unwrap();
    let rejected = store.uninstall(installed.app, false, None).unwrap();
    assert_eq!(rejected.status, Phase::Failed);
    assert!(store.application(installed.app).is_ok());
    let removed = store.uninstall(installed.app, true, None).unwrap();
    assert_eq!(removed.status, Phase::Complete);
    assert_eq!(removed.leftovers.len(), 1);
    assert!(old.exists());
}
