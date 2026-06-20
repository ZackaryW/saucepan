use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

fn saucepan(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("saucepan").unwrap();
    cmd.arg(dir.path());
    cmd
}

fn write_config(dir: &TempDir, toml: &str) {
    fs::write(dir.path().join("saucepan.toml"), toml).unwrap();
}

fn write_file(dir: &TempDir, rel: &str, content: &str) {
    let path = dir.path().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// ── config ────────────────────────────────────────────────────────────────────

#[test]
fn missing_config_errors() {
    let dir = TempDir::new().unwrap();
    saucepan(&dir)
        .arg("list")
        .assert()
        .failure()
        .stderr(contains("saucepan.toml"));
}

#[test]
fn invalid_config_errors() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "not valid toml ][");
    saucepan(&dir)
        .arg("list")
        .assert()
        .failure()
        .stderr(contains("invalid saucepan.toml"));
}

// ── list ──────────────────────────────────────────────────────────────────────

#[test]
fn list_empty_index() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(contains("no sauces installed"));
}

#[test]
fn list_shows_installed_sauce() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    write_file(
        &dir,
        ".saucepan/index.json",
        r#"[{"source_type":"local","path":"/fake","sauce":{"name":"my-lib","version":"1.0.0","description":"A test sauce"}}]"#,
    );
    saucepan(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(contains("my-lib"))
        .stdout(contains("1.0.0"))
        .stdout(contains("A test sauce"));
}

// ── bucket ────────────────────────────────────────────────────────────────────

#[test]
fn bucket_list_empty() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["bucket", "list"])
        .assert()
        .success()
        .stdout(contains("no buckets registered"));
}

#[test]
fn bucket_add_and_list() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["bucket", "add", "https://example.com/bucket.json"])
        .assert()
        .success()
        .stdout(contains("bucket added"));
    saucepan(&dir)
        .args(["bucket", "list"])
        .assert()
        .success()
        .stdout(contains("https://example.com/bucket.json"));
}

#[test]
fn bucket_add_duplicate_errors() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["bucket", "add", "https://example.com/bucket.json"])
        .assert()
        .success();
    saucepan(&dir)
        .args(["bucket", "add", "https://example.com/bucket.json"])
        .assert()
        .failure()
        .stderr(contains("already registered"));
}

#[test]
fn bucket_remove() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["bucket", "add", "https://example.com/bucket.json"])
        .assert()
        .success();
    saucepan(&dir)
        .args(["bucket", "remove", "https://example.com/bucket.json"])
        .assert()
        .success()
        .stdout(contains("bucket removed"));
    saucepan(&dir)
        .args(["bucket", "list"])
        .assert()
        .success()
        .stdout(contains("no buckets registered"));
}

#[test]
fn bucket_remove_nonexistent_errors() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["bucket", "remove", "https://example.com/bucket.json"])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

// ── search ────────────────────────────────────────────────────────────────────

#[test]
fn search_no_buckets_registered() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["search", ".name == \"foo\""])
        .assert()
        .success()
        .stdout(contains("no buckets registered"));
}

#[test]
fn search_matches_stub() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");

    let bucket_file = dir.path().join("bucket.json");
    fs::write(
        &bucket_file,
        r#"[{"name":"my-lib","version":"1.0.0","url":"https://example.com/my-lib/sauce.json"}]"#,
    )
    .unwrap();

    let bucket_url = bucket_file.to_str().unwrap().to_string();
    saucepan(&dir)
        .args(["bucket", "add", &bucket_url])
        .assert()
        .success();

    // Only run if jq is available
    if which_jq() {
        saucepan(&dir)
            .args(["search", ".name == \"my-lib\""])
            .assert()
            .success()
            .stdout(contains("my-lib"));
    }
}

#[test]
fn search_no_matches() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");

    let bucket_file = dir.path().join("bucket.json");
    fs::write(
        &bucket_file,
        r#"[{"name":"my-lib","version":"1.0.0","url":"https://example.com/my-lib/sauce.json"}]"#,
    )
    .unwrap();

    saucepan(&dir)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    if which_jq() {
        saucepan(&dir)
            .args(["search", ".name == \"nonexistent\""])
            .assert()
            .success()
            .stdout(contains("no matches"));
    }
}

// ── install (local source) ────────────────────────────────────────────────────

#[test]
fn install_already_in_local_index() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    write_file(
        &dir,
        ".saucepan/index.json",
        r#"[{"source_type":"local","path":"/fake","sauce":{"name":"my-lib","version":"1.0.0","description":"desc"}}]"#,
    );
    saucepan(&dir)
        .args(["install", "my-lib"])
        .assert()
        .success()
        .stdout(contains("already installed"));
}

#[test]
fn install_no_sources_errors() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "# no sources\n");
    saucepan(&dir)
        .args(["install", "my-lib"])
        .assert()
        .failure()
        .stderr(contains("no sources enabled"));
}

// ── update ────────────────────────────────────────────────────────────────────

#[test]
fn update_not_installed_errors() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    saucepan(&dir)
        .args(["update", "nonexistent"])
        .assert()
        .failure()
        .stderr(contains("not installed"));
}

#[test]
fn update_local_sauce_errors() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "[local]\n");
    write_file(
        &dir,
        ".saucepan/index.json",
        r#"[{"source_type":"local","path":"/fake","sauce":{"name":"my-lib","version":"1.0.0","description":"desc"}}]"#,
    );
    saucepan(&dir)
        .args(["update", "my-lib"])
        .assert()
        .failure()
        .stderr(contains("local sauces do not support update"));
}

// ── git helpers ──────────────────────────────────────────────────────────────

fn which_git() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn which_jq() -> bool {
    std::process::Command::new("jq")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a bare-minimum git repo containing a manifest file.
fn make_git_repo(manifest_name: &str, manifest_content: &str) -> TempDir {
    let repo = TempDir::new().unwrap();
    let p = repo.path();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(p)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .unwrap()
    };
    git(&["init"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["config", "user.name", "test"]);
    fs::write(p.join(manifest_name), manifest_content).unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);
    repo
}

fn default_sauce_json() -> &'static str {
    r#"{"name":"my-lib","version":"1.0.0","description":"A test sauce"}"#
}

// ── install: github source ────────────────────────────────────────────────────

#[test]
fn install_github_clones_into_github_dir() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    let repo_url = repo.path().to_str().unwrap();

    saucepan(&workspace)
        .args(["install", repo_url])
        .assert()
        .success()
        .stdout(contains("installed"));

    assert!(workspace.path().join("github").is_dir(), "github/ dir should exist");
    assert!(workspace.path().join(".saucepan/index.json").exists(), "index.json should exist");
}

#[test]
fn install_github_writes_index_entry() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());

    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();

    let idx_raw = fs::read_to_string(workspace.path().join(".saucepan/index.json")).unwrap();
    assert!(idx_raw.contains("\"source_type\": \"github\""));
    assert!(idx_raw.contains("\"name\": \"my-lib\""));
}

#[test]
fn install_github_custom_manifest_name() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("pkg.json", default_sauce_json());

    write_config(&workspace, "[github]\nbinary = \"git\"\nmanifest = \"pkg.json\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("installed"));
}

#[test]
fn install_github_missing_manifest_errors() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    // repo has no sauce.json
    let repo = make_git_repo("README.md", "hello");

    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("could not install"));
}

// ── install: customgit source ─────────────────────────────────────────────────

#[test]
fn install_customgit_clones_into_customgit_dir() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());

    // customgit base url = parent dir of the repo; name = repo dir name
    let base = repo.path().parent().unwrap().to_str().unwrap();
    let repo_name = repo.path().file_name().unwrap().to_str().unwrap();
    let toml = format!("[customgit]\nurl = \"{}\"\nbinary = \"git\"\n", base.replace('\\', "/"));
    write_config(&workspace, &toml);

    saucepan(&workspace)
        .args(["install", repo_name])
        .assert()
        .success()
        .stdout(contains("installed"));

    assert!(workspace.path().join("customgit").is_dir(), "customgit/ dir should exist");
}

// ── update: github source ─────────────────────────────────────────────────────

#[test]
fn update_github_refreshes_index() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    let repo_url = repo.path().to_str().unwrap();

    // install first
    saucepan(&workspace)
        .args(["install", repo_url])
        .assert()
        .success();

    // update the repo's sauce.json to version 2.0.0
    fs::write(
        repo.path().join("sauce.json"),
        r#"{"name":"my-lib","version":"2.0.0","description":"Updated"}"#,
    ).unwrap();
    std::process::Command::new("git")
        .args(["add", ".", "--", "sauce.json"])
        .current_dir(repo.path())
        .env("GIT_AUTHOR_NAME", "test").env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test").env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "bump version"])
        .current_dir(repo.path())
        .env("GIT_AUTHOR_NAME", "test").env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test").env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output().unwrap();

    saucepan(&workspace)
        .args(["update", "my-lib"])
        .assert()
        .success()
        .stdout(contains("updated"));

    let idx_raw = fs::read_to_string(workspace.path().join(".saucepan/index.json")).unwrap();
    assert!(idx_raw.contains("\"version\": \"2.0.0\""), "index should reflect updated version");
}

// ── search: custom jq path ────────────────────────────────────────────────────

#[test]
fn search_uses_custom_jq_path() {
    if !which_jq() { return; }
    let workspace = TempDir::new().unwrap();

    // Find actual jq path to put in config
    let jq_path = std::process::Command::new("where")
        .arg("jq")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().next().unwrap_or("jq").trim().to_string())
        .unwrap_or_else(|| "jq".to_string());

    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        r#"[{"name":"my-lib","version":"1.0.0","url":"https://example.com/sauce.json"}]"#,
    ).unwrap();

    let toml = format!(
        "jq = \"{}\"\n[local]\n",
        jq_path.replace('\\', "\\\\")
    );
    write_config(&workspace, &toml);

    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert().success();

    saucepan(&workspace)
        .args(["search", ".name == \"my-lib\""])
        .assert()
        .success()
        .stdout(contains("my-lib"));
}

// ── list: after install ───────────────────────────────────────────────────────

#[test]
fn list_shows_sauce_after_github_install() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());

    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert().success();

    saucepan(&workspace)
        .args(["list"])
        .assert()
        .success()
        .stdout(contains("my-lib"))
        .stdout(contains("1.0.0"));
}
