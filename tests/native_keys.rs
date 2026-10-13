use saucepan::core::claim::security::{KeyProvider, NativeKeyring, Secret};
mod native_support;

#[test]
#[ignore = "explicit native keyring integration"]
fn protected_manager_and_real_shim_share_native_custody() {
    if native_support::worker("protected_manager_and_real_shim_share_native_custody") {
        return;
    }
    use saucepan::{
        core::{Store, activation::Probe},
        models::{
            policy::Protection,
            source::Source,
            store::{Request, Selector},
        },
    };
    use std::{path::Path, process::Command, time::Duration};
    fn run(command: Command) -> saucepan::utils::process::Captured {
        saucepan::utils::process::run(command, Duration::from_secs(40), 64 * 1024).unwrap()
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let manager = Path::new(env!("CARGO_BIN_EXE_saucepan"));
    let shim = Path::new(env!("CARGO_BIN_EXE_saucepan-shim"));
    let mut command = Command::new(manager);
    command
        .arg("--root")
        .arg(&root)
        .args(["init", "--integrity", "keyring"]);
    let initialized = run(command);
    assert!(
        initialized.status.success(),
        "init: {}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let id = saucepan::core::claim::security::Authenticator::store_hint(
        &std::fs::read(root.join("root.json")).unwrap(),
    )
    .unwrap();
    struct Retire(uuid::Uuid);
    impl Drop for Retire {
        fn drop(&mut self) {
            if let Err(error) = NativeKeyring.retire(self.0) {
                eprintln!("fixture cleanup: {error}");
            }
        }
    }
    let cleanup = Retire(id);
    let store = Store::open(&root).unwrap();
    let source = store
        .register_source(Source::Local {
            path: manager.into(),
        })
        .unwrap();
    let name = manager.file_name().unwrap().to_str().unwrap();
    let mut request = Request {
        adopted: None,
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin.push(name.into());
    request.policies.protection = Some(Protection::UseableApp);
    let app = store.register_app("native managed app".into()).unwrap();
    let output = store.acquire(&request).unwrap();
    store
        .activate(
            app.uuid,
            output.entry.uuid,
            name,
            &Probe {
                args: vec!["--version".into()],
                stdout_contains: "saucepan ".into(),
                timeout_seconds: 10,
            },
        )
        .unwrap();
    let launcher_name = if cfg!(windows) {
        "native-test.exe"
    } else {
        "native-test"
    };
    store
        .install_shim(app.uuid, launcher_name, shim, true)
        .unwrap();
    let mut launch = Command::new(root.join("bin").join(launcher_name));
    launch.arg("--version");
    let launched = run(launch);
    assert!(
        launched.status.success(),
        "shim: {}",
        String::from_utf8_lossy(&launched.stderr)
    );
    assert!(String::from_utf8_lossy(&launched.stdout).contains("saucepan "));
    let before = std::fs::read(root.join("root.json")).unwrap();
    NativeKeyring.retire(id).unwrap();
    std::mem::forget(cleanup);
    let mut query = Command::new(manager);
    query
        .arg("--root")
        .arg(&root)
        .args(["--error-format", "json", "query-sources"]);
    let failed = run(query);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("key_unavailable"));
    assert_eq!(before, std::fs::read(root.join("root.json")).unwrap());
}
#[test]
#[ignore = "explicit native keyring integration"]
fn independent_native_store_keys_survive_concurrent_creation_and_retirement() {
    if native_support::worker(
        "independent_native_store_keys_survive_concurrent_creation_and_retirement",
    ) {
        return;
    }
    let identities: Vec<_> = (0..8).map(|_| uuid::Uuid::new_v4()).collect();
    let secrets: Vec<_> = identities
        .iter()
        .map(|_| Secret::generate().unwrap())
        .collect();
    let results = std::thread::scope(|scope| {
        let threads: Vec<_> = identities
            .iter()
            .zip(&secrets)
            .map(|(id, secret)| scope.spawn(move || NativeKeyring.insert(*id, secret)))
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    let loaded: Vec<_> = identities
        .iter()
        .map(|id| NativeKeyring.load(*id))
        .collect();
    for id in &identities {
        NativeKeyring.retire(*id).unwrap();
    }
    for ((created, loaded), secret) in results.into_iter().zip(loaded).zip(&secrets) {
        created.unwrap();
        assert!(
            loaded
                .unwrap()
                .as_ref()
                .is_some_and(|key| key.expose() == secret.expose())
        );
    }
}

#[test]
#[ignore = "explicit native keyring integration"]
fn independent_processes_initialize_distinct_stores_without_losing_keys() {
    if native_support::worker(
        "independent_processes_initialize_distinct_stores_without_losing_keys",
    ) {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let roots: Vec<_> = (0..6).map(|n| tmp.path().join(n.to_string())).collect();
    let children: Vec<_> = roots
        .iter()
        .map(|root| {
            std::process::Command::new(env!("CARGO_BIN_EXE_saucepan"))
                .arg("--root")
                .arg(root)
                .arg("init")
                .args(["--integrity", "keyring"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let results: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    let identities: Vec<_> = roots
        .iter()
        .filter_map(|root| std::fs::read(root.join("root.json")).ok())
        .map(|bytes| saucepan::core::claim::security::Authenticator::store_hint(&bytes).unwrap())
        .collect();
    let loaded: Vec<_> = identities
        .iter()
        .map(|id| NativeKeyring.load(*id))
        .collect();
    for id in &identities {
        NativeKeyring.retire(*id).unwrap();
    }
    assert_eq!(identities.len(), roots.len());
    for result in results {
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(loaded.into_iter().all(|result| result.unwrap().is_some()));
}
