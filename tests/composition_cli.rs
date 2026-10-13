use saucepan::{
    core::Store,
    models::{
        policy::UpdatePolicy,
        retrieval::{CachePolicy, RetrieveRequest, SourceInput},
        source::Source,
        store::{AcquisitionRequest, Destination, Selector},
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn save(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saucepan"));
    command
        .args(["--error-format", "json"])
        .env_remove("HOME")
        .env_remove("USERPROFILE");
    command
}
fn request(input: &Path, cache: CachePolicy) -> RetrieveRequest {
    RetrieveRequest {
        source: SourceInput::Inline {
            source: Source::Local {
                path: input.to_owned(),
            },
        },
        selector: Selector::Local,
        asset: Default::default(),
        cache,
        verification: Default::default(),
        expected_sha256: None,
    }
}
fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn consumer_args(command: &mut Command) {
    command
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "consumer_child",
            "--nocapture",
            "--skip",
            "literal space;$(no-shell)&value",
        ]);
}
#[test]
fn consumer_child() {
    let Some(report) = std::env::var_os("SAUCEPAN_TEST_CONSUMER_REPORT") else {
        return;
    };
    let report = PathBuf::from(report);
    let path = std::env::var_os("SAUCEPAN_SOURCE_PATH")
        .or_else(|| std::env::var_os("SAUCEPAN_RESOURCE_DIR"))
        .map(PathBuf::from)
        .unwrap();
    assert!(path.exists());
    assert!(std::env::args().any(|arg| arg == "literal space;$(no-shell)&value"));
    save(
        &report,
        &json!({"path":path,"kind":std::env::var("SAUCEPAN_SOURCE_KIND").ok(),"commit":std::env::var("SAUCEPAN_SOURCE_COMMIT").ok()}),
    );
    if let Some(release) = std::env::var_os("SAUCEPAN_TEST_CONSUMER_RELEASE") {
        wait_for(Path::new(&release));
    }
    assert!(path.exists(), "consumer lost its files");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    println!("consumer stdout:{input}");
    eprintln!("consumer stderr");
    fs::write(report.with_extension("done"), b"done").unwrap();
    std::process::exit(
        std::env::var("SAUCEPAN_TEST_CONSUMER_EXIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    );
}

#[test]
fn caller_receipts_work_without_root_and_cannot_forge_managed_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    fs::write(&input, b"payload").unwrap();
    let file = temp.path().join("retrieve.json");
    save(
        &file,
        &request(
            &input,
            CachePolicy::NoStore {
                destination: temp.path().join("download"),
            },
        ),
    );
    let output = cli()
        .arg("--root")
        .arg(temp.path().join("absent"))
        .arg("retrieve")
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    let acquired = temp.path().join("acquired");
    let acquire = temp.path().join("acquire.json");
    let mut value = json!({"retrieval":{"mode":"caller","request":receipt["request"],"descriptor":receipt["descriptor"]},"acquisition":{"destination":{"mode":"directory","path":acquired}}});
    save(&acquire, &value);
    let output = cli()
        .arg("acquire-retrieved")
        .arg(&acquire)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(acquired.join("input")).unwrap(), b"payload");
    assert!(!temp.path().join("absent").exists());
    value["retrieval"]["descriptor"]["ownership"] =
        json!({"kind":"managed","uuid":uuid::Uuid::new_v4()});
    value["acquisition"]["destination"]["path"] = json!(temp.path().join("forged"));
    save(&acquire, &value);
    assert!(
        !cli()
            .arg("acquire-retrieved")
            .arg(&acquire)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(!temp.path().join("forged").exists());
    save(&file, &request(&input, CachePolicy::Temporary {}));
    let output = cli().arg("retrieve").arg(&file).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run-retrieved"));
}

#[test]
fn source_configuration_lookup_purge_and_managed_acquisition_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    fs::write(&input, b"payload").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let file = temp.path().join("retrieve.json");
    save(&file, &request(&input, CachePolicy::default()));
    let output = cli()
        .arg("--root")
        .arg(store.root())
        .arg("retrieve")
        .arg(&file)
        .output()
        .unwrap();
    assert!(output.status.success());
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = receipt["descriptor"]["ownership"]["uuid"].as_str().unwrap();
    let source = receipt["descriptor"]["source"]["source_uuid"]
        .as_str()
        .unwrap();
    let output = cli()
        .arg("--root")
        .arg(store.root())
        .args(["source-path", id])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["path"],
        receipt["descriptor"]["path"]
    );
    let policy = temp.path().join("policy.json");
    save(&policy, &json!({"mode":"cache_only"}));
    assert!(
        cli()
            .arg("--root")
            .arg(store.root())
            .args(["configure-source", source])
            .arg(policy)
            .output()
            .unwrap()
            .status
            .success()
    );
    let acquire = temp.path().join("acquire.json");
    save(
        &acquire,
        &json!({"retrieval":{"mode":"managed","uuid":id},"acquisition":{}}),
    );
    let output = cli()
        .arg("--root")
        .arg(store.root())
        .arg("acquire-retrieved")
        .arg(acquire)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        cli()
            .arg("--root")
            .arg(store.root())
            .args(["purge-retrieval", id])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(Path::new(result["path"].as_str().unwrap()).exists());
}

#[test]
fn temporary_consumers_preserve_streams_literal_arguments_exit_codes_and_cleanup() {
    for acquired in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        fs::write(&input, b"payload").unwrap();
        let request_file = temp.path().join("request.json");
        let request = request(&input, CachePolicy::Temporary {});
        if acquired {
            save(
                &request_file,
                &json!({"retrieval":request,"acquisition":AcquisitionRequest { destination: Destination::Temporary {}, update_policy: UpdatePolicy::Always {}, ..Default::default() }}),
            );
        } else {
            save(&request_file, &request);
        }
        let report = temp.path().join("report.json");
        let mut command = cli();
        command
            .arg(if acquired {
                "run-acquired"
            } else {
                "run-retrieved"
            })
            .arg(&request_file)
            .env("SAUCEPAN_TEST_CONSUMER_REPORT", &report)
            .env("SAUCEPAN_TEST_CONSUMER_EXIT", "37")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        consumer_args(&mut command);
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"inherited input")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(37),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("consumer stdout:inherited input")
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("consumer stderr"));
        let report: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
        assert!(!Path::new(report["path"].as_str().unwrap()).exists());
        if !acquired {
            assert_eq!(report["kind"], "file");
        }
    }
}

// Wrapper death must not release a child's native ownership marker.
#[test]
fn children_retain_temporary_and_cached_sources_after_wrapper_death() {
    for cached in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        fs::write(&input, b"payload").unwrap();
        let store = Store::create(&temp.path().join("root")).unwrap();
        let request_file = temp.path().join("request.json");
        let mut request = request(
            &input,
            if cached {
                CachePolicy::default()
            } else {
                CachePolicy::Temporary {}
            },
        );
        let id = if cached {
            let retrieved = store.retrieve(&request).unwrap();
            let saucepan::models::retrieval::Ownership::Managed { uuid } =
                retrieved.descriptor().ownership
            else {
                panic!()
            };
            request = retrieved.request().clone();
            Some(uuid)
        } else {
            None
        };
        save(&request_file, &request);
        let report = temp.path().join("report.json");
        let release = temp.path().join("release");
        let mut command = cli();
        command
            .arg("--root")
            .arg(store.root())
            .arg("run-retrieved")
            .arg(&request_file)
            .env("SAUCEPAN_TEST_CONSUMER_REPORT", &report)
            .env("SAUCEPAN_TEST_CONSUMER_RELEASE", &release)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        consumer_args(&mut command);
        let mut wrapper = command.spawn().unwrap();
        wait_for(&report);
        // The report file can become visible just before its complete contents.
        let value: Value = loop {
            if let Ok(value) = serde_json::from_slice(&fs::read(&report).unwrap()) {
                break value;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let path = PathBuf::from(value["path"].as_str().unwrap());
        wrapper.kill().unwrap();
        wrapper.wait().unwrap();
        assert!(path.exists());
        if let Some(id) = id {
            assert!(!store.purge_retrieval(id).unwrap());
            request.cache = CachePolicy::Cached {
                freshness: Some(saucepan::models::policy::Freshness::Always {}),
                allow_stale: false,
            };
            assert!(
                store
                    .retrieve(&request)
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("child consumer")
            );
        }
        // A later consumer sweeps only closed temporary markers; live data stays.
        let sweep = temp.path().join("sweep.json");
        save(&sweep, &self::request(&input, CachePolicy::Temporary {}));
        let sweep_once = || {
            let output = cli()
                .arg("run-retrieved")
                .arg(&sweep)
                .arg("--")
                .arg(temp.path().join("missing-program"))
                .output()
                .unwrap();
            assert!(!output.status.success());
        };
        sweep_once();
        assert!(path.exists());
        fs::write(&release, b"release").unwrap();
        wait_for(&report.with_extension("done"));
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let cleaned = if let Some(id) = id {
                store.purge_retrieval(id).unwrap()
            } else {
                sweep_once();
                !path.exists()
            };
            if cleaned {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "consumer cleanup did not complete"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!path.exists());
    }
}
