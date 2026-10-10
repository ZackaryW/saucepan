use saucepan::core::claim::security::{KeyProvider, NativeKeyring, Secret};
#[test]
#[ignore = "explicit native keyring integration"]
fn independent_native_store_keys_survive_concurrent_creation_and_retirement() {
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
