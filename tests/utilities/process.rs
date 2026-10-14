use saucepan::utils::process::run;
use std::{
    io::Write,
    process::Command,
    time::{Duration, Instant},
};

fn fixture(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "process::child", "--nocapture"])
        .env("SAUCEPAN_PROCESS_FIXTURE", mode);
    command
}

#[test]
fn child() {
    let Ok(mode) = std::env::var("SAUCEPAN_PROCESS_FIXTURE") else {
        return;
    };
    match mode.as_str() {
        "capture" => {
            let thread = std::thread::spawn(|| {
                std::io::stderr().write_all(&vec![b'e'; 2_000_000]).unwrap();
            });
            std::io::stdout().write_all(&vec![b'o'; 2_000_000]).unwrap();
            thread.join().unwrap();
            std::process::exit(7);
        }
        "sleep" => {
            std::thread::sleep(Duration::from_secs(30));
        }
        "tree" => {
            let mut descendant = fixture("marker").spawn().unwrap();
            descendant.wait().unwrap();
        }
        "marker" => {
            std::thread::sleep(Duration::from_secs(1));
            std::fs::write(
                std::env::var_os("SAUCEPAN_PROCESS_MARKER").unwrap(),
                b"alive",
            )
            .unwrap();
        }
        "literal" => {
            // macOS may report /private/var for a directory opened through /var.
            // Check the resolved directory, while keeping argv checks literal.
            assert_eq!(
                std::env::current_dir().unwrap().canonicalize().unwrap(),
                std::path::PathBuf::from(std::env::var_os("SAUCEPAN_PROCESS_CWD").unwrap())
                    .canonicalize()
                    .unwrap()
            );
            assert_eq!(
                std::env::args().next_back().unwrap(),
                "space ' quote $HOME ; & literal"
            );
        }
        _ => panic!("unknown fixture"),
    }
}

#[test]
fn simultaneous_capture_exit_deadline_and_tree_cleanup() {
    let output = run(fixture("capture"), Duration::from_secs(10), 1024).unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert!(output.truncated);
    assert_eq!(output.stdout.len(), 1024);
    assert_eq!(output.stderr.len(), 1024);
    let tmp = tempfile::tempdir().unwrap();
    let marker = tmp.path().join("marker");
    let mut command = fixture("tree");
    command.env("SAUCEPAN_PROCESS_MARKER", &marker);
    let start = Instant::now();
    let error = run(command, Duration::from_millis(200), 1024).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(1200));
    assert!(!marker.exists(), "grandchild survived timeout");
}

#[test]
fn arguments_and_working_directory_are_literal() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().join("path with spaces");
    std::fs::create_dir(&cwd).unwrap();
    let mut command = fixture("literal");
    command
        .current_dir(&cwd)
        .env("SAUCEPAN_PROCESS_CWD", &cwd)
        .arg("space ' quote $HOME ; & literal");
    let output = run(command, Duration::from_secs(10), 4096).unwrap();
    assert!(
        output.status.success(),
        "child exited with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
