//! Run opt-in OS custody tests in a bounded worker, including backend prompts.
use std::{process::Command, time::Duration};

pub fn worker(name: &str) -> bool {
    if std::env::var("SAUCEPAN_NATIVE_TEST_WORKER").as_deref() == Ok(name) {
        return false;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--include-ignored", "--exact", name, "--nocapture"])
        .env("SAUCEPAN_NATIVE_TEST_WORKER", name);
    let output = saucepan::utils::process::run(command, Duration::from_secs(120), 256 * 1024)
        .unwrap_or_else(|error| panic!("native custody worker {name}: {error}"));
    assert!(
        output.status.success(),
        "native custody worker {name}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[test]
fn selected_worker_failure_is_not_a_successful_skip() {
    let name = "native_support::selected_worker_failure_is_not_a_successful_skip";
    if std::env::var("SAUCEPAN_NATIVE_TEST_WORKER").as_deref() == Ok(name) {
        panic!("injected native worker failure without contacting custody");
    }
    assert!(std::panic::catch_unwind(|| worker(name)).is_err());
}
