//! Inherited native liveness markers. No PID or elapsed-time inference.
use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

pub fn inheritable_marker(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    file.lock_shared()?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation};
        // SAFETY: the file owns this live handle throughout the call.
        unsafe {
            SetHandleInformation(
                HANDLE(file.as_raw_handle()),
                HANDLE_FLAG_INHERIT.0,
                HANDLE_FLAG_INHERIT,
            )
        }
        .map_err(io::Error::other)?;
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        // SAFETY: fcntl operates on this file's live descriptor.
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    Ok(file)
}

pub fn marker_held(path: &Path) -> io::Result<bool> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
        match options.open(path) {
            Ok(_) => Ok(false),
            Err(error) if error.raw_os_error() == Some(32) => Ok(true),
            Err(error) => Err(error),
        }
    }
    #[cfg(unix)]
    {
        let file = options.open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(false),
            Err(std::fs::TryLockError::WouldBlock) => Ok(true),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
}
