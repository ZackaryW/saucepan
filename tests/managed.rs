mod native_root;
use saucepan::{
    core::activation::Probe,
    models::{
        policy::{Eviction, Identical, Protection},
        source::Source,
        store::{Availability, Request, Selector},
    },
};
use std::{
    fs,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn fixture_probe() {
    println!("FIXTURE-HEALTHY");
}
#[test]
fn fixture_mutating_probe() {
    let path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("probe-data");
    if path.exists() {
        fs::write(path, b"changed").unwrap();
    }
    println!("FIXTURE-HEALTHY");
}

#[test]
fn probe_mutation_preserves_binding_and_corrupted_reuse_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::create_dir(&input).unwrap();
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_str().unwrap();
    fs::copy(&exe, input.join(name)).unwrap();
    fs::write(input.join("probe-data"), b"initial").unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin.push(name.into());
    request.policies.protection = Some(Protection::UseableApp);
    let app = store.register_app("probe mutation".into()).unwrap();
    let first = store.acquire(&request).unwrap();
    let probe = Probe {
        args: vec![
            "--exact".into(),
            "fixture_probe".into(),
            "--nocapture".into(),
        ],
        stdout_contains: "FIXTURE-HEALTHY".into(),
        timeout_seconds: 10,
    };
    let active = store
        .activate(app.uuid, first.entry.uuid, name, &probe)
        .unwrap();
    request.policies.identical = Identical::NewEntry;
    let next = store.acquire(&request).unwrap();
    let mutating = Probe {
        args: vec![
            "--exact".into(),
            "fixture_mutating_probe".into(),
            "--nocapture".into(),
        ],
        ..probe.clone()
    };
    let error = store
        .activate(app.uuid, next.entry.uuid, name, &mutating)
        .unwrap_err();
    assert!(
        error.to_string().contains("integrity mismatch"),
        "{error:#}"
    );
    assert_eq!(
        store
            .application(app.uuid)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        active.entry_uuid
    );
    assert!(
        store
            .activate(app.uuid, next.entry.uuid, name, &probe)
            .is_err()
    );
    // Force the only compatible reuse candidate to be corrupt as well.
    fs::write(first.directory.join("probe-data"), b"changed").unwrap();
    assert!(store.path(first.entry.uuid).is_ok());
    request.policies.identical = Identical::Reuse;
    assert!(store.acquire(&request).is_err());
}
#[test]
fn fixture_streams() {
    if std::env::var_os("SAUCEPAN_MANAGED_FIXTURE").is_none() {
        return;
    }
    use std::io::{Read, Write};
    let mut data = String::new();
    std::io::stdin().read_to_string(&mut data).unwrap();
    println!("received:{data}");
    std::io::stderr().write_all(b"fixture-stderr").unwrap();
    std::process::exit(23);
}
#[test]
fn fixture_hold() {
    let Some(base) = std::env::var_os("SAUCEPAN_MANAGED_FIXTURE") else {
        return;
    };
    let base = std::path::PathBuf::from(base);
    fs::write(base.join("ready"), b"running").unwrap();
    let start = Instant::now();
    while !base.join("release").exists() && start.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(10));
    }
    fs::write(base.join("done"), b"exited").unwrap();
}
fn until(path: &std::path::Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn deactivation_preserves_live_targets_claims_and_other_bindings() {
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_str().unwrap();
    let source = store
        .register_source(Source::Local { path: exe.clone() })
        .unwrap();
    let app = store.register_app("deactivate".into()).unwrap();
    let other = store.register_app("other".into()).unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin = vec![name.into()];
    request.policies.protection = Some(Protection::UseableApp);
    request.policies.eviction = Eviction::Latest { keep: 1 };
    request.policies.identical = Identical::NewEntry;
    let first = store
        .acquire_for(
            app.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    let entry = first.claim.entry_uuid;
    let probe = Probe {
        args: vec![
            "--exact".into(),
            "fixture_probe".into(),
            "--nocapture".into(),
        ],
        stdout_contains: "FIXTURE-HEALTHY".into(),
        timeout_seconds: 10,
    };
    store.activate(app.uuid, entry, name, &probe).unwrap();
    store.activate(other.uuid, entry, name, &probe).unwrap();
    let shim = store
        .install_shim(
            app.uuid,
            if cfg!(windows) {
                "fixture.exe"
            } else {
                "fixture"
            },
            std::path::Path::new(env!("CARGO_BIN_EXE_saucepan-shim")),
            false,
        )
        .unwrap();
    let shim_bytes = fs::read(&shim).unwrap();
    let sidecar_bytes = fs::read(shim.with_extension("shim.json")).unwrap();
    let second = store.acquire(&request).unwrap();
    let mut before = store.application(app.uuid).unwrap();
    let mut launcher = Command::new(&shim)
        .args(["--exact", "fixture_hold", "--nocapture"])
        .env("SAUCEPAN_MANAGED_FIXTURE", tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    until(&tmp.path().join("ready"));

    let maintenance_id = uuid::Uuid::new_v4();
    let maintenance = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .arg("replace-shim")
        .arg(app.uuid.to_string())
        .arg(shim.file_name().unwrap())
        .arg(env!("CARGO_BIN_EXE_saucepan-shim"))
        .arg("--operation")
        .arg(maintenance_id.to_string())
        .output()
        .unwrap();
    let maintenance_result: serde_json::Value =
        serde_json::from_slice(&maintenance.stdout).unwrap();
    if cfg!(windows) {
        assert_eq!(maintenance.status.code(), Some(2));
        assert_eq!(maintenance_result["status"], "commit_intent");
    } else {
        assert!(maintenance.status.success());
        before = store.application(app.uuid).unwrap();
    }
    let deactivate = |id: uuid::Uuid| {
        Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(store.root())
            .arg("deactivate")
            .arg(id.to_string())
            .output()
            .unwrap()
    };
    let response = deactivate(app.uuid);
    assert!(
        response.status.success(),
        "{}",
        String::from_utf8_lossy(&response.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&response.stdout).unwrap();
    assert_eq!(
        response,
        serde_json::json!({"app": app.uuid, "deactivated": true, "previous_binding": before.active})
    );
    before.active = None;
    before.revision += 1;
    assert_eq!(
        serde_json::to_value(store.application(app.uuid).unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let repeated = deactivate(app.uuid);
    assert!(repeated.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&repeated.stdout).unwrap(),
        serde_json::json!({"app": app.uuid, "deactivated": false, "previous_binding": null})
    );
    for invalid in [uuid::Uuid::new_v4(), entry] {
        let failed = deactivate(invalid);
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty());
    }
    let failed = Command::new(&shim).args(&probe.args).output().unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("application has no active binding"));
    assert!(launcher.try_wait().unwrap().is_none());
    assert!(!tmp.path().join("done").exists());
    assert_eq!(fs::read(&shim).unwrap(), shim_bytes);
    assert_eq!(
        fs::read(shim.with_extension("shim.json")).unwrap(),
        sidecar_bytes
    );

    // With both applications inactive, the running target alone protects entry.
    store.deactivate(other.uuid).unwrap();
    assert!(store.reclaim().unwrap().deferred.contains_key(&entry));
    assert!(first.acquired.directory.exists());
    // Restore the other binding before ending the process to isolate its protection.
    store.activate(other.uuid, entry, name, &probe).unwrap();
    fs::write(tmp.path().join("release"), b"stop").unwrap();
    assert!(launcher.wait().unwrap().success());
    assert!(tmp.path().join("done").exists());
    let maintained = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .args(["reconcile", "--operation"])
        .arg(maintenance_id.to_string())
        .output()
        .unwrap();
    assert!(
        maintained.status.success(),
        "{}",
        String::from_utf8_lossy(&maintained.stdout)
    );
    assert!(store.reclaim().unwrap().deferred.contains_key(&entry));
    store.deactivate(other.uuid).unwrap();
    // Deactivation itself does not reclaim even an eligible output.
    assert!(first.acquired.directory.exists());
    store.reclaim().unwrap();
    assert_eq!(store.output(entry).unwrap().state, Availability::Reclaimed);
    assert_eq!(
        store.registry(app.uuid).unwrap().claims[&first.claim.uuid].entry_uuid,
        entry
    );
    assert!(
        second.directory.exists(),
        "ordinary retention still applies"
    );
    store
        .activate(app.uuid, second.entry.uuid, name, &probe)
        .unwrap();
    let restored = Command::new(&shim).args(&probe.args).output().unwrap();
    assert!(restored.status.success());
    assert!(String::from_utf8_lossy(&restored.stdout).contains("FIXTURE-HEALTHY"));
}

#[test]
fn active_switch_failed_probe_rollback_streams_and_launcher_death() {
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_str().unwrap();
    let source = store
        .register_source(Source::Local { path: exe.clone() })
        .unwrap();
    let app = store.register_app("fixture".into()).unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin = vec![name.into()];
    request.policies.protection = Some(Protection::UseableApp);
    request.policies.eviction = Eviction::Latest { keep: 1 };
    request.policies.identical = Identical::NewEntry;
    let first = store
        .acquire_for(
            app.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    let probe = Probe {
        args: vec![
            "--exact".into(),
            "fixture_probe".into(),
            "--nocapture".into(),
        ],
        stdout_contains: "FIXTURE-HEALTHY".into(),
        timeout_seconds: 10,
    };
    store
        .activate(app.uuid, first.claim.entry_uuid, name, &probe)
        .unwrap();
    let shim = store
        .install_shim(
            app.uuid,
            if cfg!(windows) {
                "fixture.exe"
            } else {
                "fixture"
            },
            std::path::Path::new(env!("CARGO_BIN_EXE_saucepan-shim")),
            false,
        )
        .unwrap();
    let second = store
        .acquire_for(
            app.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    assert!(
        second
            .acquired
            .reclaim
            .deferred
            .contains_key(&first.claim.entry_uuid)
    );
    assert_eq!(
        store
            .application(app.uuid)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        first.claim.entry_uuid
    );
    let bad = Probe {
        stdout_contains: "MISSING".into(),
        ..probe.clone()
    };
    assert!(
        store
            .activate(app.uuid, second.claim.entry_uuid, "../outside", &probe)
            .is_err()
    );
    assert!(
        store
            .activate(app.uuid, second.claim.entry_uuid, "undeclared.exe", &probe)
            .is_err()
    );
    assert!(
        store
            .activate(app.uuid, second.claim.entry_uuid, name, &bad)
            .is_err()
    );
    assert_eq!(
        store
            .application(app.uuid)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        first.claim.entry_uuid
    );
    let mut launcher = Command::new(&shim)
        .args(["--exact", "fixture_hold", "--nocapture"])
        .env("SAUCEPAN_MANAGED_FIXTURE", tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    until(&tmp.path().join("ready"));
    store
        .activate(app.uuid, second.claim.entry_uuid, name, &probe)
        .unwrap();
    assert!(
        store
            .reclaim()
            .unwrap()
            .deferred
            .contains_key(&first.claim.entry_uuid)
    );
    // Roll back while the original output is still available and leased.
    store
        .activate(app.uuid, first.claim.entry_uuid, name, &probe)
        .unwrap();
    store
        .activate(app.uuid, second.claim.entry_uuid, name, &probe)
        .unwrap();
    #[cfg(windows)]
    {
        launcher.kill().unwrap();
        launcher.wait().unwrap();
        let report = store.reclaim().unwrap();
        assert_eq!(
            report
                .deferred
                .get(&first.claim.entry_uuid)
                .map(String::as_str),
            Some("inherited execution lifetime")
        );
    }
    fs::write(tmp.path().join("release"), b"stop").unwrap();
    until(&tmp.path().join("done"));
    #[cfg(unix)]
    launcher.wait().unwrap();
    let start = Instant::now();
    loop {
        store.reclaim().unwrap();
        if store.output(first.claim.entry_uuid).unwrap().state == Availability::Reclaimed {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        store
            .activate(app.uuid, first.claim.entry_uuid, name, &probe)
            .is_err()
    );
    let mut child = Command::new(&shim)
        .args(["--exact", "fixture_streams", "--nocapture"])
        .env("SAUCEPAN_MANAGED_FIXTURE", tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"literal text")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(23));
    assert!(String::from_utf8_lossy(&output.stdout).contains("received:literal text"));
    assert_eq!(output.stderr, b"fixture-stderr");
    let mut foreign = request.clone();
    foreign.selection.platform = Some(saucepan::models::store::Platform {
        os: "other-os".into(),
        arch: "other-arch".into(),
    });
    let foreign = store.acquire(&foreign).unwrap();
    assert!(
        store
            .activate(app.uuid, foreign.entry.uuid, name, &probe)
            .is_err()
    );
    assert_eq!(
        store
            .application(app.uuid)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        second.claim.entry_uuid
    );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows::Win32::System::Console::{
            CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent, GetConsoleCP,
        };
        // Requires an attached console; the verification run uses a PTY.
        if unsafe { GetConsoleCP() } != 0 {
            fs::remove_file(tmp.path().join("ready")).unwrap();
            fs::remove_file(tmp.path().join("release")).unwrap();
            fs::remove_file(tmp.path().join("done")).unwrap();
            let mut interrupted = Command::new(&shim)
                .args(["--exact", "fixture_hold", "--nocapture"])
                .env("SAUCEPAN_MANAGED_FIXTURE", tmp.path())
                .creation_flags(0x00000200)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            until(&tmp.path().join("ready"));
            let third = store
                .acquire_for(
                    app.uuid,
                    &"fixture".to_string().try_into().unwrap(),
                    &request,
                    &Default::default(),
                )
                .unwrap();
            store
                .activate(app.uuid, third.claim.entry_uuid, name, &probe)
                .unwrap();
            assert!(
                store
                    .reclaim()
                    .unwrap()
                    .deferred
                    .contains_key(&second.claim.entry_uuid)
            );
            unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, interrupted.id()) }.unwrap();
            assert!(!interrupted.wait().unwrap().success());
            let start = Instant::now();
            loop {
                store.reclaim().unwrap();
                if store.output(second.claim.entry_uuid).unwrap().state == Availability::Reclaimed {
                    break;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "interrupted target retained its lifetime"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            eprintln!("Windows console CTRL_BREAK forwarding verified");
        } else {
            eprintln!("Windows console interruption not executed: no attached console");
        }
    }
}
