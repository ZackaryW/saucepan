//! Literal-argument execution with bounded capture and owned process-tree cleanup.
use process_wrap::std::{ChildWrapper, CommandWrap};
use std::{
    io::{self, Read},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
}

fn drain(mut input: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut result = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 16384];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        let keep = n.min(limit.saturating_sub(result.len()));
        result.extend_from_slice(&buffer[..keep]);
        truncated |= keep < n;
    }
    Ok((result, truncated))
}

pub fn grouped(command: Command) -> CommandWrap {
    let mut command = CommandWrap::from(command);
    #[cfg(windows)]
    {
        use process_wrap::std::{CreationFlags, JobObject};
        command
            .wrap(CreationFlags(
                windows::Win32::System::Threading::CREATE_NO_WINDOW,
            ))
            .wrap(JobObject);
    }
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    command
}

struct Running(Box<dyn ChildWrapper>);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

/// Overrides stdin/stdout/stderr. The limit is per stream; excess is drained,
/// never retained. Descendants must remain in the assigned group/job. Commands
/// needing file output can use `run_to_file` without retaining those bytes.
pub fn run(mut command: Command, timeout: Duration, limit: usize) -> io::Result<Captured> {
    command.stdout(Stdio::piped());
    execute(command, timeout, limit)
}

pub fn run_to_file(
    mut command: Command,
    file: std::fs::File,
    timeout: Duration,
    limit: usize,
) -> io::Result<Captured> {
    command.stdout(file);
    execute(command, timeout, limit)
}

fn execute(mut command: Command, timeout: Duration, limit: usize) -> io::Result<Captured> {
    command.stdin(Stdio::null()).stderr(Stdio::piped());
    let mut child = Running(grouped(command).spawn()?);
    let stdout = child
        .0
        .stdout()
        .take()
        .map(|pipe| thread::spawn(move || drain(pipe, limit)));
    let stderr = thread::spawn({
        let pipe = child.0.stderr().take().unwrap();
        move || drain(pipe, limit)
    });
    let start = Instant::now();
    let result = loop {
        match child.0.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(error) => break Err(error),
            Ok(None) => {}
        }
        if start.elapsed() >= timeout {
            break Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "process deadline exceeded",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    };
    // Also stop descendants that outlive their direct parent and keep pipes open.
    let cleanup = child.0.kill();
    let out = stdout
        .map(|h| {
            h.join()
                .map_err(|_| io::Error::other("stdout reader panicked"))?
        })
        .transpose()?
        .unwrap_or_default();
    let err = stderr
        .join()
        .map_err(|_| io::Error::other("stderr reader panicked"))??;
    let status = result?;
    // On Unix ESRCH simply means the complete group already exited.
    #[cfg(unix)]
    if let Err(error) = cleanup {
        if error.raw_os_error() != Some(3) {
            return Err(error);
        }
    }
    #[cfg(not(unix))]
    cleanup?;
    Ok(Captured {
        status,
        stdout: out.0,
        stderr: err.0,
        truncated: out.1 || err.1,
    })
}
