use saucepan::{
    core::{Store, activation::Probe},
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
fn active_switch_failed_probe_rollback_streams_and_launcher_death() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
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
        .acquire_for(app.uuid, &request, &Default::default())
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
        .acquire_for(app.uuid, &request, &Default::default())
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
                .acquire_for(app.uuid, &request, &Default::default())
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
