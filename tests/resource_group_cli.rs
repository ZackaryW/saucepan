mod native_root;
mod policy_fixture;
use saucepan::{core::Store, models::source::Source};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

fn until(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "consumer fixture timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn registered_group_child() {
    let Some(tmp) = std::env::var_os("SAUCEPAN_REGISTERED_FIXTURE") else {
        return;
    };
    let tmp = PathBuf::from(tmp);
    let descriptor = PathBuf::from(std::env::var_os("SAUCEPAN_RESOURCE_GROUP_FILE").unwrap());
    let value: Value = serde_json::from_slice(&fs::read(&descriptor).unwrap()).unwrap();
    if std::env::var_os("SAUCEPAN_REGISTERED_PRINT").is_some() {
        println!(
            "{}",
            json!({"resources":value,"args":std::env::args().collect::<Vec<_>>()})
        );
        std::process::exit(23);
    }
    fs::write(tmp.join("ready"), "").unwrap();
    until(|| tmp.join("release").exists());
    for member in value["members"].as_object().unwrap().values() {
        assert_eq!(
            fs::read(Path::new(member["directory"].as_str().unwrap()).join("file")).unwrap(),
            b"original"
        );
    }
    fs::write(tmp.join("done"), "").unwrap();
}

#[test]
fn pinned_parent_reader_child() {
    let Some(base) = std::env::var_os("SAUCEPAN_PIN_READER") else {
        return;
    };
    let base = PathBuf::from(base);
    let path = PathBuf::from(std::env::var_os("SAUCEPAN_PIN_PATH").unwrap());
    // This child is not launched by Saucepan and owns no native cache guards.
    assert!(std::env::var_os("SAUCEPAN_RESOURCE_GROUP_FILE").is_none());
    fs::write(base.join("reader-ready"), "").unwrap();
    until(|| base.join("reader-release").exists());
    assert_eq!(fs::read(path.join("file")).unwrap(), b"original");
}

struct Fixture {
    tmp: tempfile::TempDir,
    store: Store,
    app: Option<Uuid>,
    group: Uuid,
    definition: PathBuf,
}
impl Fixture {
    fn new(app_owned: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let store = native_root::create(&tmp.path().join("root")).unwrap();
        let input = tmp.path().join("input");
        for name in ["command", "hooks", "service"] {
            fs::create_dir_all(input.join(name)).unwrap();
            fs::write(input.join(name).join("file"), "original").unwrap();
        }
        let source = store
            .register_source(Source::Local { path: input })
            .unwrap();
        let refs = policy_fixture::definitions(
            &store,
            vec![
                json!({"type":"update_policy","mode":"always"}),
                json!({"type":"revision_policy","history":"none"}),
            ],
        )
        .unwrap();
        let app = app_owned.then(|| store.register_app("consumer".into()).unwrap().uuid);
        let members: Vec<_> = ["command", "hooks", "service"].into_iter().map(|name| {
            let mut target = json!({"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":name},"policy_refs":refs});
            if app_owned { target["claim"] = json!({"scope":"project"}); }
            json!({"name":name,"target":target})
        }).collect();
        let definition = tmp.path().join("group.json");
        fs::write(
            &definition,
            serde_json::to_vec(&json!({"registration_key":"fixture","members":members})).unwrap(),
        )
        .unwrap();
        let mut f = Self {
            tmp,
            store,
            app,
            group: Uuid::nil(),
            definition,
        };
        let result = f.ok(&["register-group", f.definition.to_str().unwrap()]);
        f.group = serde_json::from_value(result["uuid"].clone()).unwrap();
        f
    }
    fn cli(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_saucepan"));
        cmd.arg("--root").arg(self.store.root());
        if let Some(app) = self.app {
            cmd.arg("--as-app").arg(app.to_string());
        }
        cmd.args(args);
        cmd
    }
    fn ok(&self, args: &[&str]) -> Value {
        let result = self.cli(args).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice(&result.stdout).unwrap()
    }
    fn child(&self, cmd: &mut Command) {
        cmd.args(["--"])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", "registered_group_child", "--nocapture"])
            .env("SAUCEPAN_REGISTERED_FIXTURE", self.tmp.path());
    }
}

#[test]
fn cli_registered_generation_retains_three_resources_during_update_and_removal() {
    for app in [false, true] {
        let f = Fixture::new(app);
        let id = f.group.to_string();
        assert_eq!(
            f.ok(&["list-groups", "--key", "fixture"])
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let mut wrapper = f.cli(&["prepare-run-group", &id]);
        f.child(&mut wrapper);
        let mut child = wrapper
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        until(|| f.tmp.path().join("ready").exists());
        let old = f.ok(&["show-group", &id]);
        for name in ["command", "hooks", "service"] {
            fs::write(f.tmp.path().join("input").join(name).join("file"), "next").unwrap();
        }
        let updated = f.ok(&["prepare-group", &id]);
        let revision = updated["group"]["revision"].as_u64().unwrap().to_string();
        f.ok(&["remove-group", &id, "--expected-revision", &revision]);
        let deferred = f.store.reclaim().unwrap().deferred;
        for member in old["selected"]["members"].as_object().unwrap().values() {
            assert!(deferred.contains_key(
                &serde_json::from_value::<Uuid>(member["output_uuid"].clone()).unwrap()
            ));
        }
        fs::write(f.tmp.path().join("release"), "").unwrap();
        until(|| f.tmp.path().join("done").exists());
        assert!(child.wait().unwrap().success());
        assert_eq!(f.store.reclaim().unwrap().reclaimed.len(), 3);
    }
}

#[test]
fn cli_pin_receipt_survives_exit_and_bad_inputs_never_emit_partial_success() {
    let f = Fixture::new(true);
    let id = f.group.to_string();
    let policies = policy_fixture::definitions(
        &f.store,
        vec![json!({"type":"pin_policy","retention":"until_released"})],
    )
    .unwrap();
    let pin = f.tmp.path().join("pin.json");
    fs::write(
        &pin,
        serde_json::to_vec(&json!({"registration_key":"session","policy_refs":policies})).unwrap(),
    )
    .unwrap();
    let result = f.ok(&[
        "prepare-group",
        &id,
        "--expected-revision",
        "1",
        "--pin",
        pin.to_str().unwrap(),
    ]);
    let retry = f.ok(&[
        "prepare-group",
        &id,
        "--expected-revision",
        "1",
        "--pin",
        pin.to_str().unwrap(),
    ]);
    assert_eq!(result["group"], retry["group"]);
    assert_eq!(result["pin"], retry["pin"]);
    for args in [
        vec!["prepare-group", &id, "--pin", pin.to_str().unwrap()],
        vec![
            "replace-group",
            &id,
            f.definition.to_str().unwrap(),
            "--expected-revision",
            "1",
        ],
        vec![
            "prepare-group",
            &id,
            "--expected-revision",
            "1",
            "--pin",
            f.definition.to_str().unwrap(),
        ],
    ] {
        let output = f.cli(&args).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let mut command = f.cli(&["run-registered-group", &id]);
    f.child(&mut command);
    let output = command
        .env("SAUCEPAN_REGISTERED_PRINT", "1")
        .arg("--literal-child-flag")
        .output()
        .unwrap();
    // The Rust harness rejects arbitrary arguments, proving the launcher passed it literally.
    assert!(!output.status.success());
    let mut command = f.cli(&["run-registered-group", &id]);
    f.child(&mut command);
    assert_eq!(
        command
            .env("SAUCEPAN_REGISTERED_PRINT", "1")
            .output()
            .unwrap()
            .status
            .code(),
        Some(23)
    );
    let output = f
        .cli(&[
            "run-registered-group",
            &id,
            "--",
            "saucepan-fixture-command-that-does-not-exist",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn failed_preparation_reports_member_without_success_output() {
    let f = Fixture::new(true);
    fs::remove_dir_all(f.tmp.path().join("input/hooks")).unwrap();
    let output = f
        .cli(&[
            "--error-format",
            "json",
            "prepare-group",
            &f.group.to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["details"]["members"], "hooks");
    assert!(f.store.group(f.group).unwrap().selected.is_none());
    assert!(f.store.claims(f.app.unwrap()).unwrap().is_empty());
}

#[test]
fn power_eagle_shaped_parent_reads_pinned_a_after_updater_exits_and_b_is_selected() {
    let f = Fixture::new(true);
    let id = f.group.to_string();
    let refs = policy_fixture::definitions(
        &f.store,
        vec![json!({"type":"pin_policy","retention":"until_released"})],
    )
    .unwrap();
    let pin_file = f.tmp.path().join("pin.json");
    let write_pin = |key: &str| {
        fs::write(
            &pin_file,
            serde_json::to_vec(&json!({"registration_key":key,"policy_refs":refs})).unwrap(),
        )
        .unwrap()
    };
    write_pin("plugin-host/a");
    let a = f.ok(&[
        "prepare-group",
        &id,
        "--expected-revision",
        "1",
        "--pin",
        pin_file.to_str().unwrap(),
    ]);
    let aid: Uuid =
        serde_json::from_value(a["resources"]["members"]["command"]["output_uuid"].clone())
            .unwrap();
    let mut reader = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "pinned_parent_reader_child", "--nocapture"])
        .env("SAUCEPAN_PIN_READER", f.tmp.path())
        .env(
            "SAUCEPAN_PIN_PATH",
            a["resources"]["members"]["command"]["directory"]
                .as_str()
                .unwrap(),
        )
        .env_remove("SAUCEPAN_RESOURCE_GROUP_FILE")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    until(|| f.tmp.path().join("reader-ready").exists());
    for name in ["command", "hooks", "service"] {
        fs::write(f.tmp.path().join("input").join(name).join("file"), "new").unwrap();
    }
    write_pin("plugin-host/b");
    let b = f.ok(&[
        "prepare-group",
        &id,
        "--expected-revision",
        "2",
        "--pin",
        pin_file.to_str().unwrap(),
    ]);
    assert_ne!(a["group"]["selected"], b["group"]["selected"]);
    assert_eq!(
        f.store.reclaim().unwrap().deferred[&aid],
        "durable output pin"
    );
    fs::write(f.tmp.path().join("reader-release"), "").unwrap();
    assert!(reader.wait().unwrap().success());
    f.ok(&["unpin", a["pin"]["uuid"].as_str().unwrap()]);
    assert!(f.store.reclaim().unwrap().reclaimed.contains(&aid));
}
