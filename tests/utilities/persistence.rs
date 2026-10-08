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
