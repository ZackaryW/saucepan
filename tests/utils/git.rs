use std::fs;
use tempfile::TempDir;

/// Whether the `git` binary is available on PATH.
pub fn which_git() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a git command inside `repo` with a deterministic author/committer
/// identity, and return trimmed stdout. Panics (with stderr) on failure.
pub fn git_in(repo: &TempDir, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo.path())
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// The default sauce manifest used across git-backed fixtures.
pub fn default_sauce_json() -> &'static str {
    r#"{"name":"my-lib","version":"1.0.0","description":"A test sauce"}"#
}

/// Create a bare-minimum git repo containing a manifest file.
pub fn make_git_repo(manifest_name: &str, manifest_content: &str) -> TempDir {
    let repo = TempDir::new().unwrap();
    git_in(&repo, &["init"]);
    git_in(&repo, &["config", "user.email", "test@test.com"]);
    git_in(&repo, &["config", "user.name", "test"]);
    fs::write(repo.path().join(manifest_name), manifest_content).unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "-m", "init"]);
    repo
}

/// Commit a new `sauce.json` version in `repo` and return the resulting commit hash.
pub fn commit_manifest(repo: &TempDir, version: &str) -> String {
    fs::write(
        repo.path().join("sauce.json"),
        format!(r#"{{"name":"my-lib","version":"{version}","description":"A test sauce"}}"#),
    )
    .unwrap();
    git_in(repo, &["add", "sauce.json"]);
    git_in(repo, &["commit", "-m", &format!("version {version}")]);
    git_in(repo, &["rev-parse", "HEAD"])
}
