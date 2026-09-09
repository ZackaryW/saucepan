use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

/// An exclusive native file lock released on drop. All writers must cooperate.
/// The lock file is persistent: unlinking it could let writers lock different files.
pub struct FileLock(File);

impl FileLock {
    /// Wait at most `timeout` for contention; other filesystem errors fail immediately.
    pub fn acquire(path: impl AsRef<Path>, timeout: Duration) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let start = Instant::now();
        loop {
            match fs2::FileExt::try_lock_exclusive(&file) {
                Ok(()) => return Ok(Self(file)),
                Err(error)
                    if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
                {
                    let remaining = timeout.saturating_sub(start.elapsed());
                    if remaining.is_zero() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "file lock timed out",
                        ));
                    }
                    thread::sleep(remaining.min(Duration::from_millis(10)));
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc, thread, time::Instant};

    #[test]
    fn contention_times_out_without_truncating_or_removing_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        fs::write(&path, b"keep").unwrap();
        let first = FileLock::acquire(&path, Duration::ZERO).unwrap();
        let start = Instant::now();
        let error = FileLock::acquire(&path, Duration::from_millis(25))
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(2));
        drop(first);
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        assert!(path.exists());
        FileLock::acquire(&path, Duration::ZERO).unwrap();
    }

    #[test]
    fn waiting_writer_acquires_after_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let first = FileLock::acquire(&path, Duration::ZERO).unwrap();
        let (tx, rx) = mpsc::channel();
        let writer = thread::spawn(move || {
            tx.send(()).unwrap();
            FileLock::acquire(path, Duration::from_secs(2)).unwrap()
        });
        rx.recv().unwrap();
        drop(first);
        drop(writer.join().unwrap());
    }

    #[test]
    fn missing_parent_fails_without_creating_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing/lock");
        assert_eq!(
            FileLock::acquire(&path, Duration::ZERO)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!path.parent().unwrap().exists());
    }
}
