mod native_root;
mod policy_fixture;
use saucepan::{
    core::{Store, claim::session::ApplicationRegistration, policy::catalog::PolicyCatalog},
    models::{
        lease::{GroupRequest, GroupTarget},
        source::Source,
        store::{Availability, Request, Selector},
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

fn until(mut condition: impl FnMut() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "fixture timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn group_child() {
    let Some(base) = std::env::var_os("SAUCEPAN_GROUP_FIXTURE") else {
        return;
    };
    let base = PathBuf::from(base);
    let mode = std::env::var("SAUCEPAN_GROUP_MODE").unwrap();
    if mode == "unrelated" {
        fs::write(base.join("ready"), "").unwrap();
        until(|| base.join("release").exists());
        return;
    }
    for key in [
        "SAUCEPAN_RESOURCE_DIR",
        "SAUCEPAN_SOURCE_PATH",
        "SAUCEPAN_SOURCE_KIND",
        "SAUCEPAN_SOURCE_COMMIT",
    ] {
        assert!(std::env::var_os(key).is_none());
    }
    let path = PathBuf::from(std::env::var_os("SAUCEPAN_RESOURCE_GROUP_FILE").unwrap());
    assert!(path.is_absolute());
    let descriptor: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(descriptor["members"].as_object().unwrap().len(), 4);
    fs::write(base.join("ready"), path.to_str().unwrap()).unwrap();
    if mode == "streams" {
        use std::io::{Read, Write};
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap();
        println!(
            "GROUP:{}",
            json!({"descriptor":descriptor,"cwd":std::env::current_dir().unwrap(),"args":std::env::args().collect::<Vec<_>>(),"stdin":input})
        );
        std::io::stderr().write_all(b"group stderr").unwrap();
        std::process::exit(23);
    }
    until(|| base.join("release").exists());
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
        descriptor
    );
    for (name, value) in descriptor["members"].as_object().unwrap() {
        let relative = if name == "hook_files" {
            "resource"
        } else {
            "files/resource"
        };
        assert_eq!(
            fs::read(Path::new(value["directory"].as_str().unwrap()).join(relative)).unwrap(),
            b"original"
        );
    }
    fs::write(base.join("done"), "").unwrap();
}
struct Fixture {
    tmp: tempfile::TempDir,
    store: Store,
    requests: Vec<Request>,
    ids: Vec<Uuid>,
    group: GroupRequest,
}
impl Fixture {
    fn new() -> Self {
        Self::with_default_root(false)
    }
    fn with_default_root(default: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let store = native_root::create(&tmp.path().join(if default {
            ".saucepan"
        } else {
            "store with space λ"
        }))
        .unwrap();
        let mut requests = Vec::new();
        let mut ids = Vec::new();
        let mut members = Vec::new();
        for name in ["command", "hooks", "service"] {
            let input = tmp.path().join(name);
            fs::create_dir_all(input.join("files")).unwrap();
            fs::write(input.join("files/resource"), "original").unwrap();
            let source = store
                .register_source(Source::Local { path: input })
                .unwrap();
            let mut request = Request {
                source_uuid: source.uuid,
                selector: Selector::Local,
                selection: Default::default(),
                policy_refs: Default::default(),
            };
            policy_fixture::replace(
                &store,
                &mut request.policy_refs,
                json!({"type":"revision_policy","history":"latest","keep":1}),
            )
            .unwrap();
            let id = store.acquire(&request).unwrap().entry.uuid;
            ids.push(id);
            requests.push(request);
            members.push(json!({"name":name,"target":{"type":"output","uuid":id}}));
        }
        members.push(json!({"name":"hook_files","target":{"type":"output","uuid":ids[1]},"relative":"files"}));
        let group = serde_json::from_value(json!({"members":members})).unwrap();
        Self {
            tmp,
            store,
            requests,
            ids,
            group,
        }
    }
    fn wrapper(&self, app: Option<Uuid>) -> Command {
        let file = self.tmp.path().join("group.json");
        fs::write(&file, serde_json::to_vec(&self.group).unwrap()).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_saucepan"));
        command.arg("--root").arg(self.store.root());
        if let Some(app) = app {
            command.arg("--as-app").arg(app.to_string());
        }
        command.arg("run-group").arg(file);
        command
    }
    fn child(&self, command: &mut Command, mode: &str) {
        command
            .arg("--")
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", "group_child", "--nocapture"])
            .env("SAUCEPAN_GROUP_FIXTURE", self.tmp.path())
            .env("SAUCEPAN_GROUP_MODE", mode);
    }
    fn replace_outputs(&self) {
        for (name, request) in ["command", "hooks", "service"].iter().zip(&self.requests) {
            fs::write(self.tmp.path().join(name).join("files/resource"), "updated").unwrap();
            self.store.acquire(request).unwrap();
        }
    }
    fn no_live_markers(&self) {
        self.store.reclaim().unwrap();
        assert!(
            !fs::read_dir(self.store.root().join(".locks"))
                .unwrap()
                .any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("live-"))
        );
    }
}

#[test]
fn literal_child_environment_streams_paths_and_exit_status() {
    use std::io::Write;
    let mut f = Fixture::new();
    let leaf = json!({"uuid":Uuid::new_v4(),"type":"verification_policy","policy_target":"path","mode":"full"});
    f.group.policy_refs = vec![serde_json::from_value(leaf["uuid"].clone()).unwrap()]
        .try_into()
        .unwrap();
    let policies = f.tmp.path().join("portable.json");
    fs::write(&policies, serde_json::to_vec(&vec![leaf]).unwrap()).unwrap();
    let parent = std::env::var_os("SAUCEPAN_RESOURCE_GROUP_FILE");
    let mut command = f.wrapper(None);
    command.arg("--policies").arg(policies);
    f.child(&mut command, "streams");
    let literal = "spaces $VALUE $(not-a-command) `backticks` λ";
    command
        .args(["--skip", literal])
        .current_dir(f.tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in [
        "SAUCEPAN_RESOURCE_DIR",
        "SAUCEPAN_SOURCE_PATH",
        "SAUCEPAN_SOURCE_KIND",
        "SAUCEPAN_SOURCE_COMMIT",
        "SAUCEPAN_RESOURCE_GROUP_FILE",
    ] {
        command.env(key, "stale value");
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"input bytes")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(23), "{:?}", result);
    assert_eq!(result.stderr, b"group stderr");
    let text = String::from_utf8(result.stdout).unwrap();
    let value: Value =
        serde_json::from_str(text.lines().find_map(|s| s.strip_prefix("GROUP:")).unwrap()).unwrap();
    assert_eq!(value["stdin"], "input bytes");
    assert_eq!(value["args"].as_array().unwrap().last().unwrap(), literal);
    assert_eq!(
        Path::new(value["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        f.tmp.path().canonicalize().unwrap()
    );
    let members = &value["descriptor"]["members"];
    assert!(
        members
            .as_object()
            .unwrap()
            .values()
            .all(|m| m["verification"] == "full")
    );
    assert_eq!(
        members["hooks"]["output_uuid"],
        members["hook_files"]["output_uuid"]
    );
    assert_eq!(
        Path::new(members["hook_files"]["directory"].as_str().unwrap()),
        Path::new(members["hooks"]["directory"].as_str().unwrap()).join("files")
    );
    assert!(!text.contains("credential"));
    assert_eq!(std::env::var_os("SAUCEPAN_RESOURCE_GROUP_FILE"), parent);
    f.no_live_markers();
}

#[test]
fn run_group_uses_default_home_store_without_app_or_credentials() {
    let f = Fixture::with_default_root(true);
    fs::write(
        f.tmp.path().join("group.json"),
        serde_json::to_vec(&f.group).unwrap(),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_saucepan"));
    command
        .env("HOME", f.tmp.path())
        .env("USERPROFILE", f.tmp.path())
        .arg("run-group")
        .arg(f.tmp.path().join("group.json"));
    f.child(&mut command, "streams");
    let result = command.stdin(Stdio::null()).output().unwrap();
    assert_eq!(result.status.code(), Some(23), "{:?}", result);
    f.no_live_markers();
}

#[test]
fn three_resource_zush_groups_survive_updates_release_and_wrapper_termination() {
    for scoped in [false, true] {
        let mut f = Fixture::new();
        let mut claims = Vec::new();
        let app = if scoped {
            let app = f.store.register_app("Zush fixture".into()).unwrap().uuid;
            for (index, request) in f.requests.iter().enumerate() {
                let claim = f
                    .store
                    .acquire_for(
                        app,
                        &"workspace".to_string().try_into().unwrap(),
                        request,
                        &Default::default(),
                    )
                    .unwrap()
                    .claim
                    .uuid;
                claims.push(claim);
                f.group.members[index].target = GroupTarget::Claim { uuid: claim };
            }
            f.group.members[3].target = GroupTarget::Claim { uuid: claims[1] };
            let policy=serde_json::from_value(json!({"uuid":Uuid::new_v4(),"type":"access_policy","scopes":["workspace"],"actions":["lease"]})).unwrap();
            let receipt = f.store.register_policies(&[policy]).unwrap();
            f.group.policy_refs = receipt.try_into().unwrap();
            Some(app)
        } else {
            None
        };
        let mut command = f.wrapper(app);
        f.child(&mut command, "hold");
        let mut child = command
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        until(|| f.tmp.path().join("ready").exists());
        let descriptor = PathBuf::from(fs::read_to_string(f.tmp.path().join("ready")).unwrap());
        for (index, claim) in claims.iter().enumerate() {
            fs::write(
                f.tmp
                    .path()
                    .join(["command", "hooks", "service"][index])
                    .join("files/resource"),
                "updated",
            )
            .unwrap();
            f.store
                .update_claim(app.unwrap(), *claim, &Default::default())
                .unwrap();
            f.store.release_claim(app.unwrap(), *claim).unwrap();
        }
        f.replace_outputs();
        #[cfg(windows)]
        {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        let report = f.store.reclaim().unwrap();
        for id in &f.ids {
            assert!(report.deferred.contains_key(id));
        }
        assert!(descriptor.exists());
        fs::write(f.tmp.path().join("release"), "").unwrap();
        until(|| f.tmp.path().join("done").exists());
        #[cfg(unix)]
        assert!(child.wait().unwrap().success());
        until(|| {
            f.store.reclaim().unwrap();
            f.ids
                .iter()
                .all(|id| f.store.output(*id).unwrap().state == Availability::Reclaimed)
        });
        f.no_live_markers();
        // A later consumer sweep removes only closed descriptor scratch ownership.
        let mut latest = f.group.clone();
        latest.policy_refs = Default::default();
        for (i, member) in latest.members.iter_mut().enumerate() {
            let index = if i == 3 { 1 } else { i };
            member.target = GroupTarget::Output {
                uuid: f.store.acquire(&f.requests[index]).unwrap().entry.uuid,
            };
        }
        f.group = latest;
        let failed = f
            .wrapper(None)
            .arg("--")
            .arg(f.tmp.path().join("absent-program"))
            .output()
            .unwrap();
        assert!(!failed.status.success());
        assert!(!descriptor.exists());
    }
}

#[test]
fn denied_and_oversized_requests_never_start_consumers() {
    let mut f = Fixture::new();
    let app = f
        .store
        .register_application(
            &ApplicationRegistration {
                registration_key: "protected".to_string().try_into().unwrap(),
                name: "protected".into(),
                credentials_required: true,
                policy_refs: Default::default(),
            },
            None,
        )
        .unwrap();
    for mode in [
        "claim-no-app",
        "unknown",
        "protected",
        "oversized",
        "oversized-catalog",
        "deny",
        "shadow",
        "missing-program",
    ] {
        f.group = serde_json::from_value(
            json!({"members":[{"name":"resource","target":{"type":"output","uuid":f.ids[0]}}]}),
        )
        .unwrap();
        if mode == "claim-no-app" {
            f.group.members[0].target = GroupTarget::Claim {
                uuid: Uuid::new_v4(),
            };
        }
        if mode == "unknown" {
            f.group.members[0].target = GroupTarget::Output {
                uuid: Uuid::new_v4(),
            };
        }
        let mut command = f.wrapper((mode == "protected").then_some(app.application.uuid));
        command.args(["--error-format", "json"]);
        if mode == "oversized" {
            fs::write(f.tmp.path().join("group.json"), vec![b' '; 1024 * 1024 + 1]).unwrap();
        }
        if mode == "oversized-catalog" {
            let path = f.tmp.path().join("oversized.json");
            fs::write(&path, vec![b' '; 1024 * 1024 + 1]).unwrap();
            command.arg("--policies").arg(path);
        }
        if mode == "deny" || mode == "shadow" {
            let leaf = json!({"uuid":Uuid::new_v4(),"type":"access_policy","actions":[]});
            f.group.policy_refs = vec![serde_json::from_value(leaf["uuid"].clone()).unwrap()]
                .try_into()
                .unwrap();
            fs::write(
                f.tmp.path().join("group.json"),
                serde_json::to_vec(&f.group).unwrap(),
            )
            .unwrap();
            let policies = f.tmp.path().join("policies.json");
            fs::write(&policies, serde_json::to_vec(&vec![leaf.clone()]).unwrap()).unwrap();
            if mode == "shadow" {
                f.store
                    .register_policies(&[serde_json::from_value(leaf).unwrap()])
                    .unwrap();
            }
            command.arg("--policies").arg(policies);
        }
        if mode == "missing-program" {
            command.arg("--").arg(f.tmp.path().join("absent-program"));
        } else {
            f.child(&mut command, "hold");
        }
        let result = command.output().unwrap();
        assert!(!result.status.success(), "{mode}");
        assert!(result.stdout.is_empty());
        let error: Value = serde_json::from_slice(&result.stderr).unwrap();
        let expected = match mode {
            "claim-no-app" | "deny" => "access_denied",
            "unknown" => "unknown_output",
            "protected" => "authentication_required",
            "missing-program" => "io_error",
            _ => "invalid_request",
        };
        assert_eq!(error["error"]["code"], expected, "{mode}: {error}");
        assert!(!f.tmp.path().join("ready").exists());
        f.no_live_markers();
    }
}

#[test]
fn native_groups_are_not_inherited_by_unrelated_children() {
    let f = Fixture::new();
    let lease = f
        .store
        .lease_group(&f.group, &PolicyCatalog::default())
        .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "group_child", "--nocapture"])
        .env("SAUCEPAN_GROUP_FIXTURE", f.tmp.path())
        .env("SAUCEPAN_GROUP_MODE", "unrelated")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    until(|| f.tmp.path().join("ready").exists());
    f.replace_outputs();
    drop(lease);
    f.store.reclaim().unwrap();
    assert!(
        f.ids
            .iter()
            .all(|id| f.store.output(*id).unwrap().state == Availability::Reclaimed)
    );
    fs::write(f.tmp.path().join("release"), "").unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn group_console_or_signal_interruption_releases_all_outputs() {
    #[cfg(windows)]
    if unsafe { windows::Win32::System::Console::GetConsoleCP() } == 0 {
        eprintln!("Group interruption not executed: no attached console");
        return;
    }
    let f = Fixture::new();
    let mut command = f.wrapper(None);
    f.child(&mut command, "hold");
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
    until(|| f.tmp.path().join("ready").exists());
    f.replace_outputs();
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
        unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) }.unwrap();
    }
    #[cfg(unix)]
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    assert!(!child.wait().unwrap().success());
    until(|| {
        f.store.reclaim().unwrap();
        f.ids
            .iter()
            .all(|id| f.store.output(*id).unwrap().state == Availability::Reclaimed)
    });
    f.no_live_markers();
}
