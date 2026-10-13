mod native_root;
use saucepan::{
    core::Store,
    models::{
        policy::{Identical, RevisionPolicy},
        source::Source,
        store::{Availability, Request, Selector, VerificationMode},
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn consumer_fixture() {
    let Some(base) = std::env::var_os("SAUCEPAN_CONSUMER_FIXTURE") else {
        return;
    };
    let base = PathBuf::from(base);
    let mode = std::env::var("SAUCEPAN_CONSUMER_MODE").unwrap();
    if mode == "streams" {
        use std::io::{Read, Write};
        let root = PathBuf::from(std::env::var_os("SAUCEPAN_RESOURCE_DIR").unwrap());
        assert_eq!(fs::read(root.join("data")).unwrap(), b"payload");
        let mut stdin = String::new();
        std::io::stdin().read_to_string(&mut stdin).unwrap();
        let value = serde_json::json!({"directory":root,"args":std::env::args().collect::<Vec<_>>(),"cwd":std::env::current_dir().unwrap(),"stdin":stdin});
        println!("CONSUMER:{value}");
        std::io::stderr().write_all(b"consumer-stderr").unwrap();
        std::process::exit(23);
    }
    fs::write(base.join("ready"), b"ready").unwrap();
    let start = Instant::now();
    while !base.join("release").exists() {
        assert!(start.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(10));
    }
    if mode == "hold" {
        let root = PathBuf::from(std::env::var_os("SAUCEPAN_RESOURCE_DIR").unwrap());
        assert_eq!(fs::read(root.join("data")).unwrap(), b"payload");
    }
    fs::write(base.join("done"), b"done").unwrap();
}

fn until(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn setup() -> (tempfile::TempDir, native_root::NativeStore, Request) {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("data");
    fs::write(&input, b"payload").unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut request = Request {
        adopted: None,
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.policies.identical = Identical::NewEntry;
    request.policies.revision = RevisionPolicy::Latest { keep: 1 };
    (tmp, store, request)
}

fn markers(store: &Store) -> usize {
    fs::read_dir(store.root().join(".locks"))
        .unwrap()
        .filter(|p| {
            p.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("live-")
        })
        .count()
}

#[test]
fn retained_process_marker_survives_wrapper_death_then_is_swept() {
    let (tmp, store, request) = setup();
    let first = store.acquire(&request).unwrap();
    let mut command = wrapper(&store, first.entry.uuid);
    fixture(&mut command, tmp.path(), "hold");
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    until(&tmp.path().join("ready"));
    assert_eq!(markers(&store), 1);
    store.reclaim().unwrap();
    assert_eq!(markers(&store), 1);
    #[cfg(windows)]
    {
        child.kill().unwrap();
        child.wait().unwrap();
        store.reclaim().unwrap();
        assert_eq!(
            markers(&store),
            1,
            "inherited marker removed with live target"
        );
    }
    fs::write(tmp.path().join("release"), b"release").unwrap();
    until(&tmp.path().join("done"));
    #[cfg(unix)]
    assert!(child.wait().unwrap().success());
    let start = Instant::now();
    while markers(&store) != 0 {
        store.reclaim().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        store.output(first.entry.uuid).unwrap().state,
        Availability::Ready
    );
    assert_eq!(fs::read(first.directory.join("data")).unwrap(), b"payload");
}
fn wrapper(store: &Store, entry: uuid::Uuid) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saucepan"));
    command
        .arg("--root")
        .arg(store.root())
        .arg("run")
        .arg(entry.to_string());
    command
}
fn fixture(command: &mut Command, tmp: &Path, mode: &str) {
    command
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "consumer_fixture", "--nocapture"])
        .env("SAUCEPAN_CONSUMER_FIXTURE", tmp)
        .env("SAUCEPAN_CONSUMER_MODE", mode);
}

#[test]
fn literal_arguments_environment_streams_cwd_and_status() {
    use std::io::Write;
    let (tmp, store, request) = setup();
    let output = store.acquire(&request).unwrap();
    let inherited = std::env::var_os("SAUCEPAN_RESOURCE_DIR");
    let mut command = wrapper(&store, output.entry.uuid);
    command.args(["--verification", "full"]);
    fixture(&mut command, tmp.path(), "streams");
    let literal = "spaces $VALUE $(not-a-command) `backticks` --root other";
    command
        .args(["--skip", literal])
        .env("SAUCEPAN_RESOURCE_DIR", "must-be-overridden")
        .current_dir(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"input bytes")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(23), "{:?}", result);
    assert_eq!(result.stderr, b"consumer-stderr");
    let stdout = String::from_utf8(result.stdout).unwrap();
    let line = stdout
        .lines()
        .find_map(|s| s.strip_prefix("CONSUMER:"))
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(value["directory"], output.directory.to_str().unwrap());
    assert_eq!(
        Path::new(value["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        tmp.path().canonicalize().unwrap()
    );
    assert_eq!(value["stdin"], "input bytes");
    assert_eq!(value["args"].as_array().unwrap().last().unwrap(), literal);
    assert!(!stdout.contains("\"verification\""));
    assert_eq!(std::env::var_os("SAUCEPAN_RESOURCE_DIR"), inherited);
}

#[test]
fn consumer_survives_claim_release_retention_and_wrapper_death() {
    let (tmp, store, request) = setup();
    let app = store.register_app("consumer".into()).unwrap();
    let first = store
        .acquire_for(
            app.uuid,
            &"fixture".to_string().try_into().unwrap(),
            &request,
            &Default::default(),
        )
        .unwrap();
    let id = first.claim.entry_uuid;
    let mut command = wrapper(&store, id);
    fixture(&mut command, tmp.path(), "hold");
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    until(&tmp.path().join("ready"));
    store.release_claim(app.uuid, first.claim.uuid).unwrap();
    assert!(
        store
            .acquire(&request)
            .unwrap()
            .reclaim
            .deferred
            .contains_key(&id)
    );
    #[cfg(windows)]
    {
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(
            store
                .reclaim()
                .unwrap()
                .deferred
                .get(&id)
                .map(String::as_str),
            Some("inherited execution lifetime")
        );
    }
    fs::write(tmp.path().join("release"), b"release").unwrap();
    until(&tmp.path().join("done"));
    #[cfg(unix)]
    assert!(child.wait().unwrap().success());
    let start = Instant::now();
    while store.output(id).unwrap().state != Availability::Reclaimed {
        store.reclaim().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn failed_launch_or_verification_does_not_leave_live_ownership() {
    let (tmp, store, request) = setup();
    for case in ["missing", "corrupt", "unknown-mode", "missing-program"] {
        let output = store.acquire(&request).unwrap();
        let mut command = wrapper(&store, output.entry.uuid);
        match case {
            "missing" => {
                command.arg("--").arg(tmp.path().join("nonexistent"));
            }
            "corrupt" => {
                fs::write(output.directory.join("data"), b"corrupt").unwrap();
                command.args(["--verification", "full"]);
                fixture(&mut command, tmp.path(), "hold");
            }
            "unknown-mode" => {
                command.args(["--verification", "unknown"]);
                fixture(&mut command, tmp.path(), "hold");
            }
            _ => (),
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        assert!(!tmp.path().join("ready").exists());
        assert!(result.stdout.is_empty());
        store.reclaim().unwrap();
        assert_eq!(
            markers(&store),
            0,
            "failed run left stale markers after reclaim"
        );
        assert!(
            store
                .acquire(&request)
                .unwrap()
                .reclaim
                .reclaimed
                .contains(&output.entry.uuid)
        );
    }
}

#[test]
fn native_guard_is_not_inherited_by_unrelated_spawn() {
    let (tmp, store, request) = setup();
    let first = store.acquire(&request).unwrap();
    let guard = store
        .lease(first.entry.uuid, VerificationMode::Metadata)
        .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "consumer_fixture", "--nocapture"])
        .env("SAUCEPAN_CONSUMER_FIXTURE", tmp.path())
        .env("SAUCEPAN_CONSUMER_MODE", "unrelated")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    until(&tmp.path().join("ready"));
    drop(guard);
    assert!(child.try_wait().unwrap().is_none());
    assert!(
        store
            .acquire(&request)
            .unwrap()
            .reclaim
            .reclaimed
            .contains(&first.entry.uuid)
    );
    fs::write(tmp.path().join("release"), b"release").unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn consumer_console_or_signal_interruption() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::GetConsoleCP;
        if unsafe { GetConsoleCP() } == 0 {
            eprintln!("Run interruption not executed: no attached console");
            return;
        }
    }
    let (tmp, store, request) = setup();
    let first = store.acquire(&request).unwrap();
    let mut command = wrapper(&store, first.entry.uuid);
    fixture(&mut command, tmp.path(), "hold");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000200);
    }
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    until(&tmp.path().join("ready"));
    assert!(
        store
            .acquire(&request)
            .unwrap()
            .reclaim
            .deferred
            .contains_key(&first.entry.uuid)
    );
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
        unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) }.unwrap();
    }
    #[cfg(unix)]
    {
        // SAFETY: signal only this test's live child process.
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    }
    assert!(!child.wait().unwrap().success());
    let start = Instant::now();
    while store.output(first.entry.uuid).unwrap().state != Availability::Reclaimed {
        store.reclaim().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!("Leased run console/signal interruption verified");
}
