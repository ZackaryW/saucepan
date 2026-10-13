use saucepan::{
    core::Store,
    models::{
        policy::{
            Freshness, PolicyRecord,
            requests::{Acquisition, Retrieval},
        },
        retrieval::{CachePolicy, SourceInput},
        source::Source,
        store::{Destination, Selector},
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
fn cli(temp: &tempfile::TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saucepan"));
    command
        .args(["--error-format", "json"])
        .env_remove("HOME")
        .env_remove("USERPROFILE");
    let catalog = temp.path().join("portable-policies.json");
    if catalog.exists() {
        command.arg("--policies").arg(catalog);
    }
    command
}
fn definition(directory: &Path, mut value: Value) -> uuid::Uuid {
    let path = directory.join("portable-policies.json");
    let mut records: Vec<Value> = if path.exists() {
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap()
    } else {
        vec![]
    };
    let id = uuid::Uuid::parse_str(
        &saucepan::core::claim::persistence::digest(&serde_json::to_vec(&value).unwrap())[..32],
    )
    .unwrap();
    value["uuid"] = json!(id);
    if !records.contains(&value) {
        records.push(value);
        save(&path, &records);
    }
    id
}
fn catalog(temp: &tempfile::TempDir) -> saucepan::core::policy::catalog::PolicyCatalog {
    let path = temp.path().join("portable-policies.json");
    saucepan::core::policy::catalog::PolicyCatalog::new(if path.exists() {
        serde_json::from_slice::<Vec<PolicyRecord>>(&fs::read(path).unwrap()).unwrap()
    } else {
        vec![]
    })
    .unwrap()
}
fn set_cache(request: &mut Retrieval, cache: CachePolicy, directory: &Path) {
    let mut refs = vec![];
    request.destination = None;
    match cache {
        CachePolicy::Cached {
            freshness,
            allow_stale,
        } => {
            if let Some(freshness) = freshness {
                let mut value = serde_json::to_value(freshness).unwrap();
                value["type"] = json!("refresh_policy");
                refs.push(definition(directory, value));
            }
            if allow_stale {
                refs.push(definition(
                    directory,
                    json!({"type":"stale_policy","allow_stale":true}),
                ));
            }
        }
        CachePolicy::Temporary {} => refs.push(definition(
            directory,
            json!({"type":"cache_policy","mode":"temporary"}),
        )),
        CachePolicy::NoStore { destination } => {
            request.destination = Some(destination);
            refs.push(definition(
                directory,
                json!({"type":"cache_policy","mode":"no_store"}),
            ));
        }
    }
    request.policy_refs = refs.try_into().unwrap();
}
fn request(input: &Path, cache: CachePolicy) -> Retrieval {
    let mut request = Retrieval {
        source: SourceInput::Inline {
            source: Source::Local {
                path: input.to_owned(),
            },
        },
        selector: Selector::Local,
        asset: Default::default(),
        policy_refs: Default::default(),
        destination: None,
        expected_sha256: None,
    };
    set_cache(&mut request, cache, input.parent().unwrap());
    request
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
    if let Ok(kind) = std::env::var("SAUCEPAN_TEST_KIND") {
        match kind.as_str() {
            "zip" => assert!(fs::read(&path).unwrap().starts_with(b"PK")),
            "directory" => assert_eq!(fs::read(path.join("file")).unwrap(), b"fresh"),
            "git_repository" => {
                assert_eq!(std::env::var("SAUCEPAN_SOURCE_KIND").unwrap(), kind);
                let commit = std::env::var("SAUCEPAN_SOURCE_COMMIT").unwrap();
                let output = Command::new("git")
                    .arg("-C")
                    .arg(&path)
                    .args(["show", &format!("{commit}:selected/file")])
                    .output()
                    .unwrap();
                assert!(output.status.success());
                assert_eq!(output.stdout, b"fresh");
            }
            _ => panic!("unexpected representation"),
        }
    }
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
    let output = cli(&temp)
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
    let output = cli(&temp)
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
        !cli(&temp)
            .arg("acquire-retrieved")
            .arg(&acquire)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(!temp.path().join("forged").exists());
    save(&file, &request(&input, CachePolicy::Temporary {}));
    let output = cli(&temp).arg("retrieve").arg(&file).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run-retrieved"));
}

#[test]
fn referenced_cli_rejects_inline_settings_and_catalog_shadowing_before_effects() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    fs::write(&input, b"payload").unwrap();
    let request = request(
        &input,
        CachePolicy::NoStore {
            destination: temp.path().join("download"),
        },
    );
    let file = temp.path().join("request.json");
    for field in ["cache", "verification", "provenance"] {
        let mut mixed = serde_json::to_value(&request).unwrap();
        mixed[field] = json!({});
        save(&file, &mixed);
        let output = cli(&temp).arg("retrieve").arg(&file).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!temp.path().join("download").exists());
    }
    let store = Store::create(&temp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let definitions: Vec<PolicyRecord> =
        serde_json::from_slice(&fs::read(temp.path().join("portable-policies.json")).unwrap())
            .unwrap();
    store.register_policies(&definitions).unwrap();
    let mut request = request;
    request.source = SourceInput::Registered {
        source_uuid: source.uuid,
    };
    save(&file, &request);
    let before = saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap();
    let output = cli(&temp)
        .arg("--root")
        .arg(store.root())
        .arg("retrieve")
        .arg(&file)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!temp.path().join("download").exists());
    assert_eq!(
        before,
        saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap()
    );
    // With persistent definitions only, fresh retrieval and receipt reopening
    // are observational even when the native caller holds a normal Store.
    let retrieved = store
        .retrieve_referenced(&request, &Default::default(), &Default::default())
        .unwrap();
    let descriptor = retrieved.descriptor().clone();
    drop(retrieved);
    let reopened = store
        .open_caller_referenced(&request, &descriptor, &Default::default(), None)
        .unwrap();
    assert_eq!(fs::read(reopened.source_path()).unwrap(), b"payload");
    drop(reopened);
    assert_eq!(
        before,
        saucepan::utils::tree::inspect(store.root(), Default::default()).unwrap()
    );
}

#[test]
fn source_configuration_lookup_purge_and_managed_acquisition_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    fs::write(&input, b"payload").unwrap();
    let store = Store::create(&temp.path().join("root")).unwrap();
    let file = temp.path().join("retrieve.json");
    save(&file, &request(&input, CachePolicy::default()));
    let output = cli(&temp)
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
    let output = cli(&temp)
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
    let policy_id = uuid::Uuid::new_v4();
    let record: PolicyRecord = serde_json::from_value(
        json!({"uuid":policy_id,"type":"refresh_policy","mode":"cache_only"}),
    )
    .unwrap();
    store.register_policies(&[record]).unwrap();
    save(
        &policy,
        &json!({"expected_revision":0,"policy_refs":[policy_id]}),
    );
    assert!(
        cli(&temp)
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
    let output = cli(&temp)
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
        cli(&temp)
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
                &json!({"retrieval":request,"acquisition":Acquisition { destination: Destination::Temporary {}, policy_refs: vec![definition(temp.path(), json!({"type":"update_policy","mode":"always"}))].try_into().unwrap(), ..Default::default() }}),
            );
        } else {
            save(&request_file, &request);
        }
        let report = temp.path().join("report.json");
        let mut command = cli(&temp);
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
            let retrieved = store
                .retrieve_referenced(&request, &Default::default(), &Default::default())
                .unwrap();
            let saucepan::models::retrieval::Ownership::Managed { uuid } =
                retrieved.descriptor().ownership
            else {
                panic!()
            };
            request.source = retrieved.descriptor().source.clone();
            Some(uuid)
        } else {
            None
        };
        save(&request_file, &request);
        let report = temp.path().join("report.json");
        let release = temp.path().join("release");
        let mut command = cli(&temp);
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
            set_cache(
                &mut request,
                CachePolicy::Cached {
                    freshness: Some(Freshness::Always {}),
                    allow_stale: false,
                },
                temp.path(),
            );
            store
                .register_policies(&catalog(&temp).records().cloned().collect::<Vec<_>>())
                .unwrap();
            assert!(
                store
                    .retrieve_referenced(&request, &Default::default(), &Default::default())
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
            let output = cli(&temp)
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
fn snapshot(path: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, result: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = Default::default();
    walk(path, path, &mut result);
    result
}
#[test]
fn fresh_zip_and_git_consumers_ignore_preexisting_caches_without_home() {
    for repository in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp
            .path()
            .join(if repository { "origin" } else { "bundle.zip" });
        if repository {
            fs::create_dir(&input).unwrap();
            git(&input, &["init", "-b", "main"]);
            fs::create_dir(input.join("selected")).unwrap();
            fs::write(input.join("selected/file"), b"cached").unwrap();
            git(&input, &["add", "."]);
            git(&input, &["commit", "-m", "cached"]);
        } else {
            let mut zip = zip::ZipWriter::new(fs::File::create(&input).unwrap());
            zip.start_file("selected/file", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"cached").unwrap();
            zip.finish().unwrap();
        }
        let store = Store::create(&temp.path().join("root")).unwrap();
        let mut retrieval = request(&input, CachePolicy::default());
        if repository {
            retrieval.source = SourceInput::Inline {
                source: Source::Git {
                    origin: input.to_str().unwrap().into(),
                },
            };
            retrieval.selector = Selector::Ref {
                reference: "main".into(),
            };
        }
        drop(
            store
                .retrieve_referenced(&retrieval, &Default::default(), &Default::default())
                .unwrap(),
        );
        let before = snapshot(store.root());
        if repository {
            fs::write(input.join("selected/file"), b"fresh").unwrap();
            git(&input, &["commit", "-am", "fresh"]);
        } else {
            let mut zip = zip::ZipWriter::new(fs::File::create(&input).unwrap());
            zip.start_file("selected/file", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"fresh").unwrap();
            zip.finish().unwrap();
        }
        for caller in [false, true] {
            for acquired in [false, true] {
                let suffix = format!("{caller}-{acquired}");
                set_cache(
                    &mut retrieval,
                    if caller {
                        CachePolicy::NoStore {
                            destination: temp.path().join(format!("download-{suffix}")),
                        }
                    } else {
                        CachePolicy::Temporary {}
                    },
                    temp.path(),
                );
                let file = temp.path().join(format!("request-{suffix}.json"));
                if acquired {
                    save(
                        &file,
                        &json!({"retrieval":retrieval,"acquisition":{
                            "destination":{"mode":"temporary"}, "folder":"selected", "format": if repository { "natural" } else { "zip" }
                        }}),
                    );
                } else {
                    save(&file, &retrieval);
                }
                let report = temp.path().join(format!("report-{suffix}.json"));
                let mut command = cli(&temp);
                command
                    .arg("--root")
                    .arg(store.root())
                    .arg(if acquired {
                        "run-acquired"
                    } else {
                        "run-retrieved"
                    })
                    .arg(&file)
                    .env("SAUCEPAN_TEST_CONSUMER_REPORT", &report)
                    .env(
                        "SAUCEPAN_TEST_KIND",
                        if acquired {
                            "directory"
                        } else if repository {
                            "git_repository"
                        } else {
                            "zip"
                        },
                    )
                    .stdin(Stdio::null());
                consumer_args(&mut command);
                let output = command.output().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let value: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
                assert_eq!(
                    Path::new(value["path"].as_str().unwrap()).exists(),
                    caller && !acquired
                );
                assert_eq!(
                    snapshot(store.root()),
                    before,
                    "fresh operation changed a managed cache"
                );
            }
        }
        // An unavailable fresh origin must not fall back to the populated cache.
        fs::rename(&input, temp.path().join("unavailable")).unwrap();
        set_cache(&mut retrieval, CachePolicy::Temporary {}, temp.path());
        let file = temp.path().join("failure.json");
        save(&file, &retrieval);
        let output = cli(&temp)
            .arg("--root")
            .arg(store.root())
            .arg("run-retrieved")
            .arg(file)
            .args(["--", "unused"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(snapshot(store.root()), before);
    }
}
