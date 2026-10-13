use saucepan::utils::{
    fs::{atomic_write, staged_directory},
    lock::{LockGuard, LockMode},
    path::relative_path,
};
use std::{fs, io::Write, process::Command, time::Duration};

#[test]
fn portable_paths() {
    for path in [
        "", "/a", "../a", "a/../b", "C:/a", "a\\b", "a//b", "a:stream", "CON", "a.", "a\0",
    ] {
        assert!(relative_path(path).is_err(), "{path:?}");
    }
    assert!(relative_path("folder/é file.json").is_ok());
}
#[test]
fn publication_preserves_old_data_on_failure() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    fs::write(&file, "old").unwrap();
    assert!(
        atomic_write(&file, |w| {
            w.write_all(b"partial")?;
            Err(std::io::Error::other("injected"))
        })
        .is_err()
    );
    assert_eq!(fs::read(&file).unwrap(), b"old");
    atomic_write(&file, |w| {
        assert_eq!(fs::read(&file)?, b"old");
        w.write_all(b"new")
    })
    .unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"new");
    let dir = root.path().join("directory");
    assert!(
        staged_directory(&dir, |p| {
            fs::write(p.join("partial"), "x")?;
            Err(std::io::Error::other("injected"))
        })
        .is_err()
    );
    assert!(!dir.exists());
    staged_directory(&dir, |p| fs::write(p.join("complete"), "ok")).unwrap();
    assert!(staged_directory(&dir, |_| panic!("occupied destination")).is_err());
    assert!(atomic_write(&dir, |_| Ok(())).is_err());
    assert!(atomic_write(&root.path().join("missing/file"), |_| Ok(())).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
}
#[test]
fn lock_child() {
    if let Some(path) = std::env::var_os("SAUCEPAN_TEST_LOCK") {
        assert!(
            LockGuard::acquire(
                std::path::Path::new(&path),
                LockMode::Exclusive,
                Duration::from_millis(30)
            )
            .is_err()
        );
    }
}
#[test]
fn locks_coordinate_processes_and_release() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("lock");
    let one = LockGuard::acquire(&path, LockMode::Shared, Duration::ZERO).unwrap();
    let two = LockGuard::acquire(&path, LockMode::Shared, Duration::ZERO).unwrap();
    assert!(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "persistence::lock_child"])
            .env("SAUCEPAN_TEST_LOCK", &path)
            .status()
            .unwrap()
            .success()
    );
    drop(one);
    drop(two);
    let _exclusive = LockGuard::acquire(&path, LockMode::Exclusive, Duration::ZERO).unwrap();
}

#[test]
fn delivery_child() {
    if let Some(root) = std::env::var_os("SAUCEPAN_TEST_DELIVERY") {
        let root = std::path::PathBuf::from(root);
        let id = std::env::var("SAUCEPAN_TEST_DELIVERY_ID").unwrap();
        let result = saucepan::utils::fs::staged_directory_coordinated(
            &root.join("destination"),
            &root.join("delivery.lock"),
            Duration::from_secs(10),
            |stage| fs::write(stage.join("winner"), &id),
        );
        let outcome = match result {
            Ok(()) => "published",
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => "occupied",
            Err(error) => panic!("{error}"),
        };
        fs::write(root.join(id), outcome).unwrap();
    }
}

#[test]
fn cooperating_deliveries_preserve_the_winner() {
    let root = tempfile::tempdir().unwrap();
    let mut children = Vec::new();
    for id in ["a", "b"] {
        children.push(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "persistence::delivery_child"])
                .env("SAUCEPAN_TEST_DELIVERY", root.path())
                .env("SAUCEPAN_TEST_DELIVERY_ID", id)
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let mut results: Vec<_> = ["a", "b"]
        .map(|id| fs::read_to_string(root.path().join(id)).unwrap())
        .into();
    results.sort();
    assert_eq!(results, ["occupied", "published"]);
    let winner = fs::read_to_string(root.path().join("destination/winner")).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join(winner)).unwrap(),
        "published"
    );
}

#[test]
fn destination_preflight_rejects_overlap_and_preserves_occupied_paths() {
    use saucepan::utils::path::disjoint_destination;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    assert!(disjoint_destination(&source, &source.join("inside")).is_err());
    assert!(disjoint_destination(&source, root.path()).is_err());
    assert!(disjoint_destination(&source, &source).is_err());
    let destination = root.path().join("out");
    assert_eq!(
        disjoint_destination(&source, &destination).unwrap(),
        root.path().canonicalize().unwrap().join("out")
    );
    fs::write(&destination, b"occupied").unwrap();
    assert!(disjoint_destination(&source, &destination).is_err());
    assert_eq!(fs::read(destination).unwrap(), b"occupied");
}
