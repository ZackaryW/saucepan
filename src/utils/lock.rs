use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub enum LockMode {
    Shared,
    Exclusive,
}

pub struct LockGuard(File);
impl LockGuard {
    pub fn acquire(path: &Path, mode: LockMode, timeout: Duration) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        let start = Instant::now();
        loop {
            let result = match mode {
                LockMode::Shared => file.try_lock_shared(),
                LockMode::Exclusive => file.try_lock(),
            };
            match result {
                Ok(()) => return Ok(Self(file)),
                Err(std::fs::TryLockError::Error(e)) => return Err(e),
                Err(std::fs::TryLockError::WouldBlock) if start.elapsed() >= timeout => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "lock timeout"));
                }
                Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(
                    Duration::from_millis(5).min(timeout.saturating_sub(start.elapsed())),
                ),
            }
        }
    }
}
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
