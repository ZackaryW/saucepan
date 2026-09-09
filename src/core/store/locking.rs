use super::layout::{Layout, io_error};
use crate::core::models::{Error, ErrorKind, Result};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::Path,
    time::{Duration, Instant},
};

pub(super) struct Lock {
    file: File,
}
impl Lock {
    /// One order across every mutation: bindings, sources, then central. Callers
    /// must discover dependencies before acquiring this complete set.
    pub(super) fn transaction(
        layout: &Layout,
        sources: impl IntoIterator<Item = String>,
        bindings: impl IntoIterator<Item = String>,
        deadline: Instant,
    ) -> Result<Vec<Self>> {
        let mut names = std::collections::BTreeSet::new();
        for (prefix, ids) in [
            ("source", sources.into_iter().collect::<Vec<_>>()),
            ("binding", bindings.into_iter().collect::<Vec<_>>()),
        ] {
            for id in ids {
                if id.is_empty() || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "invalid resource lock identity",
                    ));
                }
                names.insert(format!("{prefix}-{id}"));
            }
        }
        let mut locks = Vec::with_capacity(names.len() + 1);
        for name in names {
            locks.push(Self::acquire(layout, &name, deadline)?);
        }
        locks.push(Self::acquire(layout, "central", deadline)?);
        Ok(locks)
    }
    pub(super) fn acquire(layout: &Layout, name: &str, deadline: Instant) -> Result<Self> {
        Self::acquire_mode(layout, name, deadline, false)
    }
    pub(super) fn shared(layout: &Layout, name: &str, deadline: Instant) -> Result<Self> {
        Self::acquire_mode(layout, name, deadline, true)
    }
    fn acquire_mode(layout: &Layout, name: &str, deadline: Instant, shared: bool) -> Result<Self> {
        if name.is_empty() || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err(Error::new(ErrorKind::Integrity, "invalid lock identity"));
        }
        let path = layout.path(Path::new(&format!("locks/{name}.lock")))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| io_error("cannot open store lock", e))?;
        loop {
            match if shared {
                FileExt::try_lock_shared(&file)
            } else {
                FileExt::try_lock_exclusive(&file)
            } {
                Ok(()) => return Ok(Self { file }),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
                {
                    if Instant::now() >= deadline {
                        return Err(Error::new(
                            ErrorKind::Busy,
                            "store resource is busy; retry within a new deadline",
                        ));
                    }
                    std::thread::sleep(
                        Duration::from_millis(10)
                            .min(deadline.saturating_duration_since(Instant::now())),
                    );
                }
                Err(e) => return Err(io_error("cannot lock store", e)),
            }
        }
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn worker(home: &Path, mode: &str) -> std::process::Command {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "core::store::locking::tests::process_worker",
                "--ignored",
            ])
            .env("SAUCEPAN_LOCK_TEST_HOME", home)
            .env("SAUCEPAN_LOCK_TEST_MODE", mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        command
    }
    #[test]
    #[ignore = "subprocess fixture, invoked by process_locking"]
    fn process_worker() {
        let Some(home) = std::env::var_os("SAUCEPAN_LOCK_TEST_HOME") else {
            return;
        };
        let mode = std::env::var("SAUCEPAN_LOCK_TEST_MODE").unwrap();
        if mode == "archive-readers" {
            let layout = Layout::at_test_root(Path::new(&home)).unwrap();
            let names = std::env::var("SAUCEPAN_ARCHIVE_LEASE_TEST_NAMES").unwrap();
            let _leases: Vec<_> = names
                .lines()
                .map(|name| Lock::shared(&layout, name, Instant::now()).unwrap())
                .collect();
            std::fs::write(layout.root().join("readers-ready"), b"ready").unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            while !layout.root().join("readers-release").exists() {
                assert!(Instant::now() < deadline, "reader fixture release deadline");
                std::thread::sleep(Duration::from_millis(10));
            }
            return;
        }
        let layout = Layout::under(Path::new(&home)).unwrap();
        if mode == "busy" {
            assert!(
                matches!(Lock::transaction(&layout, vec!["a".into()], vec!["unit".into()], Instant::now()), Err(e) if e.kind == ErrorKind::Busy)
            );
            return;
        }
        let sources = if mode == "reverse" {
            vec!["b".into(), "a".into()]
        } else {
            vec!["a".into(), "b".into(), "a".into()]
        };
        let _locks = Lock::transaction(
            &layout,
            sources,
            vec!["unit".into()],
            Instant::now() + Duration::from_secs(3),
        )
        .unwrap();
        let counter = layout.read(Path::new("counter"), 16).unwrap();
        let counter: u32 = std::str::from_utf8(&counter).unwrap().parse().unwrap();
        std::thread::sleep(Duration::from_millis(30));
        layout
            .replace(Path::new("counter"), (counter + 1).to_string().as_bytes())
            .unwrap();
    }
    #[test]
    fn process_locking_orders_resources_and_preserves_both_updates() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        layout.create_file(Path::new("counter"), b"0").unwrap();
        let lock = Lock::acquire(&layout, "source-a", Instant::now()).unwrap();
        let output = worker(home.path(), "busy").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        drop(lock);
        let forward = worker(home.path(), "forward").spawn().unwrap();
        let reverse = worker(home.path(), "reverse").spawn().unwrap();
        for child in [forward, reverse] {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert_eq!(layout.read(Path::new("counter"), 16).unwrap(), b"2");
        // A failed partial acquisition released its binding lock as well.
        Lock::transaction(
            &layout,
            vec!["a".into()],
            vec!["unit".into()],
            Instant::now(),
        )
        .unwrap();
    }
    #[test]
    fn contention_is_bounded_and_lock_releases() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        let lock = Lock::acquire(&layout, "central", Instant::now()).unwrap();
        let second = Lock::acquire(&layout, "central", Instant::now());
        assert!(matches!(second, Err(e) if e.kind == ErrorKind::Busy));
        drop(lock);
        Lock::acquire(&layout, "central", Instant::now()).unwrap();
    }
}
