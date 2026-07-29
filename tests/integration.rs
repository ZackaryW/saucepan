use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

mod utils;
use utils::git::{commit_manifest, default_sauce_json, git_in, make_git_repo, which_git};
use utils::index::{read_buckets_json, read_index_json, read_index_text};

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
        .code(3)
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

// ── jq helper ────────────────────────────────────────────────────────────────

fn which_jq() -> bool {
    std::process::Command::new("jq")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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

    let idx_raw = read_index_text(&workspace);
    assert!(idx_raw.contains("\"source_type\": \"github\""));
    assert!(idx_raw.contains("\"name\": \"my-lib\""));
}

#[test]
fn install_github_records_default_branch_commit() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let expected = git_in(&repo, &["rev-parse", "HEAD"]);
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert!(idx[0].get("reference").is_none());
}

#[test]
fn install_github_branch_ref_records_branch_head() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    git_in(&repo, &["checkout", "-b", "feature"]);
    let expected = commit_manifest(&repo, "2.0.0");
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap(), "--ref", "feature"])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["reference"], "feature");
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert_eq!(idx[0]["sauce"]["version"], "2.0.0");
}

#[test]
fn install_github_tag_ref_reads_tagged_manifest() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let expected = git_in(&repo, &["rev-parse", "HEAD"]);
    git_in(&repo, &["tag", "v1.0.0"]);
    commit_manifest(&repo, "2.0.0");
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap(), "--ref", "v1.0.0"])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["reference"], "v1.0.0");
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
}

#[test]
fn install_github_commit_ref_reads_pinned_manifest() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let expected = git_in(&repo, &["rev-parse", "HEAD"]);
    commit_manifest(&repo, "2.0.0");
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap(), "--ref", &expected])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["reference"], expected);
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
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
        .code(1)
        .stderr(contains("could not install"));
}

#[test]
fn install_backend_launch_failure_returns_source_error() {
    let workspace = TempDir::new().unwrap();
    write_config(&workspace, "[github]\nbinary = \"gh\"\n");
    let empty_path = workspace.path().join("empty-path");
    fs::create_dir(&empty_path).unwrap();

    saucepan(&workspace)
        .env("PATH", &empty_path)
        .args(["install", "owner/repo"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("could not install"))
        .stderr(contains("github source"));
}

#[test]
fn install_falls_back_from_github_to_customgit() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let base = repo.path().parent().unwrap().to_str().unwrap().replace('\\', "/");
    let repo_name = repo.path().file_name().unwrap().to_str().unwrap();
    write_config(
        &workspace,
        &format!(
            "[github]\nbinary = \"git\"\n[customgit]\nurl = \"{base}\"\nbinary = \"git\"\n"
        ),
    );

    saucepan(&workspace)
        .args(["install", repo_name])
        .assert()
        .success()
        .stdout(contains("from customgit"));
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
    git_in(&repo, &["add", ".", "--", "sauce.json"]);
    git_in(&repo, &["commit", "-m", "bump version"]);

    saucepan(&workspace)
        .args(["update", "my-lib"])
        .assert()
        .success()
        .stdout(contains("updated"));

    let idx_raw = read_index_text(&workspace);
    assert!(idx_raw.contains("\"version\": \"2.0.0\""), "index should reflect updated version");
}

#[test]
fn update_github_partial_destination_is_replaced() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();

    let checkout = fs::read_dir(workspace.path().join("github"))
        .unwrap().next().unwrap().unwrap().path();
    fs::remove_dir_all(checkout.join(".git")).unwrap();
    assert!(checkout.exists(), "destination should remain, minus .git");

    saucepan(&workspace)
        .args(["update", "my-lib"])
        .assert()
        .success()
        .stdout(contains("updated"));

    assert!(
        checkout.join(".git").is_dir(),
        "partial destination should be cleaned up and replaced with a fresh clone"
    );
}

#[test]
fn install_raw_git_with_token_warns_once_and_uses_native_credentials() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    write_config(&workspace, "[github]\nbinary = \"git\"\ntoken = \"ignored-secret\"\n");

    let output = saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.matches("native Git credentials").count(), 1, "stderr: {stderr}");
}

#[test]
fn update_github_branch_ref_advances_and_refreshes_revision() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    git_in(&repo, &["checkout", "-b", "feature"]);
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap(), "--ref", "feature"])
        .assert()
        .success();
    let expected = commit_manifest(&repo, "2.0.0");

    saucepan(&workspace)
        .args(["update", "my-lib"])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["reference"], "feature");
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert_eq!(idx[0]["sauce"]["version"], "2.0.0");
}

#[test]
fn update_github_tag_ref_stays_on_tagged_commit() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let expected = git_in(&repo, &["rev-parse", "HEAD"]);
    git_in(&repo, &["tag", "v1.0.0"]);
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap(), "--ref", "v1.0.0"])
        .assert()
        .success();
    commit_manifest(&repo, "2.0.0");

    saucepan(&workspace)
        .args(["update", "my-lib"])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
}

#[test]
fn update_github_commit_ref_remains_pinned() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let expected = git_in(&repo, &["rev-parse", "HEAD"]);
    write_config(&workspace, "[github]\nbinary = \"git\"\n");

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap(), "--ref", &expected])
        .assert()
        .success();
    commit_manifest(&repo, "2.0.0");

    saucepan(&workspace)
        .args(["update", "my-lib"])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["resolved_commit"], expected);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
}

// ── uninstall ────────────────────────────────────────────────────────────────

#[test]
fn uninstall_github_removes_index_entry_and_managed_checkout() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();
    let checkout = fs::read_dir(workspace.path().join("github"))
        .unwrap().next().unwrap().unwrap().path();

    saucepan(&workspace)
        .args(["uninstall", "my-lib"])
        .assert()
        .success()
        .stdout(contains("uninstalled my-lib"));

    assert!(!checkout.exists());
    assert_eq!(read_index_text(&workspace), "[]");
}

#[test]
fn uninstall_customgit_removes_index_entry_and_managed_checkout() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let base = repo.path().parent().unwrap().to_str().unwrap().replace('\\', "/");
    let repo_name = repo.path().file_name().unwrap().to_str().unwrap();
    write_config(&workspace, &format!("[customgit]\nurl = \"{base}\"\nbinary = \"git\"\n"));
    saucepan(&workspace)
        .args(["install", repo_name])
        .assert()
        .success();
    let checkout = fs::read_dir(workspace.path().join("customgit"))
        .unwrap().next().unwrap().unwrap().path();

    saucepan(&workspace)
        .args(["uninstall", "my-lib"])
        .assert()
        .success();

    assert!(!checkout.exists());
    assert_eq!(read_index_text(&workspace), "[]");
}

#[test]
fn uninstall_local_removes_only_index_entry() {
    let workspace = TempDir::new().unwrap();
    let local = TempDir::new().unwrap();
    write_config(&workspace, "[local]\n");
    write_file(
        &workspace,
        ".saucepan/index.json",
        &format!(
            r#"[{{"source_type":"local","path":"{}","sauce":{{"name":"my-lib","version":"1.0.0","description":"desc"}}}}]"#,
            local.path().to_str().unwrap().replace('\\', "\\\\")
        ),
    );

    saucepan(&workspace)
        .args(["uninstall", "my-lib"])
        .assert()
        .success();

    assert!(local.path().exists());
    assert_eq!(read_index_text(&workspace), "[]");
}

#[test]
fn uninstall_missing_managed_checkout_removes_stale_entry() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();
    fs::remove_dir_all(workspace.path().join("github")).unwrap();

    saucepan(&workspace)
        .args(["uninstall", "my-lib"])
        .assert()
        .success();

    assert_eq!(read_index_text(&workspace), "[]");
}

#[test]
fn uninstall_unknown_returns_not_found_and_preserves_index() {
    let workspace = TempDir::new().unwrap();
    write_config(&workspace, "[local]\n");
    write_file(
        &workspace,
        ".saucepan/index.json",
        r#"[{"source_type":"local","path":"/fake","sauce":{"name":"existing","version":"1.0.0","description":"desc"}}]"#,
    );
    let before = read_index_text(&workspace);

    saucepan(&workspace)
        .args(["uninstall", "missing"])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("not installed"));

    assert_eq!(read_index_text(&workspace), before);
}

// ── search: custom jq path ────────────────────────────────────────────────────

#[test]
fn search_uses_custom_jq_path() {
    if !which_jq() { return; }
    let workspace = TempDir::new().unwrap();

    // Find actual jq path to put in config. The locator binary differs by
    // platform: `where` is Windows-only, `which` is the POSIX equivalent.
    let locator = if cfg!(windows) { "where" } else { "which" };
    let jq_path = std::process::Command::new(locator)
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

// ── manifest resolution chain ─────────────────────────────────────────────────
//
// `fetch_sauce` (src/sources/git.rs) resolves a manifest through an ordered
// chain: the target's own root manifest, then registered central indexes.
// `install`/`update` already thread `root` all the way into `fetch_sauce`, so
// the chain activates for them with no further wiring — these tests exercise
// it through the real CLI, the same way every other install/update behavior
// in this file is exercised.

/// Escape a native path for embedding as a JSON string value, matching the
/// pattern already used by `uninstall_local_removes_only_index_entry` above.
fn json_escape_path(path: &std::path::Path) -> String {
    path.to_str().unwrap().replace('\\', "\\\\")
}

#[test]
fn install_uses_index_supplied_manifest_when_repo_has_none() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    // The target repo carries no sauce.json at all.
    let repo = make_git_repo("README.md", "hello");
    let head = git_in(&repo, &["rev-parse", "HEAD"]);
    let target = json_escape_path(repo.path());

    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"1.0.0","description":"from index"}},"ref":"{head}"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("installed"));

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["sauce"]["name"], "my-lib");
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
    assert_eq!(idx[0]["resolved_commit"], head);
}

#[test]
fn install_repository_manifest_wins_over_index_and_index_is_never_consulted() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let repo_head = git_in(&repo, &["rev-parse", "HEAD"]);
    git_in(&repo, &["tag", "v1"]);
    // Advance the repo past the tag; the repository's own manifest must win
    // regardless, so the recorded commit must be the *current* HEAD, not v1.
    let advanced_head = commit_manifest(&repo, "3.0.0");
    let target = json_escape_path(repo.path());

    // An index also describes this exact target, with a different manifest
    // and a ref pointing at the earlier, tagged commit.
    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"9.9.9","description":"from index"}},"ref":"v1"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    // Repository's own manifest, not the index's 9.9.9.
    assert_eq!(idx[0]["sauce"]["version"], "3.0.0");
    // HEAD unaffected by the index's ref — proves the index link was never consulted.
    assert_eq!(idx[0]["resolved_commit"], advanced_head);
    assert_ne!(idx[0]["resolved_commit"], repo_head);
}

/// Task 6.1 — the single most important compatibility test in the change:
/// a manifest-bearing install must produce an entry, identity, and
/// resolved commit byte-identical to what pre-chain `fetch_sauce` produced,
/// even when a registered index also describes the exact same target. This
/// is constructed to fail loudly if the repository link were ever
/// accidentally bypassed in favor of the index link, or if the
/// resolved-commit computation changed: the competing index entry uses a
/// different name, version, and a ref pointing at a *different* commit, so
/// any leakage from the index into the installed entry is directly
/// observable.
#[test]
fn install_manifest_bearing_repo_is_unchanged_by_a_registered_competing_index() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("sauce.json", default_sauce_json());
    let repo_head = git_in(&repo, &["rev-parse", "HEAD"]);
    // Advance the repo so a second, distinct commit exists for the index's
    // ref to (wrongly) point at if it were ever consulted.
    let advanced_head = commit_manifest(&repo, "1.0.0-advanced-not-installed");
    assert_ne!(repo_head, advanced_head);
    let target = json_escape_path(repo.path());

    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"impostor","version":"9.9.9","url":"{target}","manifest":{{"name":"impostor","version":"9.9.9","description":"must never be installed"}},"ref":"{repo_head}"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    let output = saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    // The repository's own manifest resolves before the index link is ever
    // reached, so the chain must produce no index-related diagnostics here.
    assert!(!stderr.contains("index"), "stderr: {stderr}");
    assert!(!stderr.contains("shadowed"), "stderr: {stderr}");

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["source_type"], "github");
    assert_eq!(idx[0]["sauce"]["name"], "my-lib");
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0-advanced-not-installed");
    assert_eq!(idx[0]["sauce"]["description"], "A test sauce");
    // Resolved commit is the repo's own current HEAD, never the index's
    // pinned (and now stale) ref.
    assert_eq!(idx[0]["resolved_commit"], advanced_head);
    assert_ne!(idx[0]["resolved_commit"], repo_head);
    // Provenance correctly attributes the manifest to the repository, not
    // the index that also happened to describe this target.
    assert_eq!(idx[0]["manifest_source"], serde_json::json!({"kind": "repository"}));
    assert!(idx[0].get("reference").is_none());
}

#[test]
fn install_index_entry_without_ref_is_rejected_and_chain_falls_to_not_found() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("README.md", "hello");
    let target = json_escape_path(repo.path());

    // Supplies a manifest but no ref — invalid per the spec, must be
    // rejected rather than accepted or treated as a hard failure.
    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"1.0.0","description":"from index"}}}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("could not install"));
}

/// Task 6.2 — with no index registered at all, the chain has exactly one
/// effective link (`RepositoryManifestLink`), and the design's migration
/// plan claims this reduces to "behavior identical to today." Assert that
/// literally, down to the exact pre-chain `NotFound` message text
/// (`"no {manifest} found in {repo_url}"`, unchanged since before this
/// change per `src/sources/git.rs`), not just the exit code — a chain that
/// silently changed wording, or that emitted extra diagnostics from
/// consulting an (absent) index registry, would still pass a looser check.
#[test]
fn install_missing_manifest_with_no_index_registered_reports_pre_chain_error_text_verbatim() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("README.md", "hello");
    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    // No `bucket add` at all — buckets.json does not even exist.

    let output = saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1), "chain exhaustion must still be exit code 1");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let expected = format!("no sauce.json found in {}", repo.path().to_str().unwrap());
    assert!(stderr.contains(&expected), "stderr: {stderr}");
    assert!(!workspace.path().join(".saucepan/buckets.json").exists());
}

/// Task 6.3 — installing a manifest-less repository through an
/// index-supplied manifest and pinned ref must record `resolved_commit` as
/// the entry's pinned ref, not whatever the target's current HEAD happens
/// to be. The target is advanced past the pinned commit after registration
/// so a HEAD-based (rather than ref-based) resolved-commit computation
/// would be caught directly.
#[test]
fn install_index_supplied_manifest_pins_resolved_commit_to_the_ref_not_current_head() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("README.md", "hello");
    let pinned_commit = git_in(&repo, &["rev-parse", "HEAD"]);

    // Advance the repo past the pinned commit. It still carries no
    // sauce.json, so `RepositoryManifestLink` still has nothing to offer —
    // only the index-supplied ref should determine the resolved commit.
    fs::write(repo.path().join("README.md"), "hello again").unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "-m", "advance past the pinned ref"]);
    let current_head = git_in(&repo, &["rev-parse", "HEAD"]);
    assert_ne!(pinned_commit, current_head);
    let target = json_escape_path(repo.path());

    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"1.0.0","description":"from index"}},"ref":"{pinned_commit}"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success();

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
    assert_eq!(idx[0]["resolved_commit"], pinned_commit);
    assert_ne!(idx[0]["resolved_commit"], current_head);
    assert_eq!(
        idx[0]["manifest_source"],
        serde_json::json!({"kind": "index", "index": bucket_file.to_str().unwrap()})
    );
}

/// Regression test for the central-index link's stub-to-target matching:
/// it used to require byte-for-byte string equality (`stub.url != repo_url`)
/// between a registered index's stub `url` and the install target, so a
/// stub naming the exact same repository with a trivially different
/// spelling — here, a trailing slash the install target lacks — silently
/// failed to match. That produced no error at all: it looked identical to
/// "no index describes this target," and the manifest-less repo fell
/// through to `NotFound`. Under the old exact-equality comparison this
/// install would fail outright; it must now succeed and resolve the
/// manifest through the index.
#[test]
fn install_matches_index_stub_url_differing_from_target_by_a_trailing_slash() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    // The target repo carries no sauce.json at all, so it can only be
    // installed via an index-supplied manifest.
    let repo = make_git_repo("README.md", "hello");
    let head = git_in(&repo, &["rev-parse", "HEAD"]);
    let target = json_escape_path(repo.path());

    // The stub's url is the target *plus* a trailing slash — a trivial
    // spelling difference the fix's normalized comparison must see through.
    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}/","manifest":{{"name":"my-lib","version":"1.0.0","description":"from index"}},"ref":"{head}"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("installed"));

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["sauce"]["name"], "my-lib");
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
    assert_eq!(idx[0]["resolved_commit"], head);
    // Proves the manifest genuinely resolved through the index link, not
    // some other path.
    assert_eq!(
        idx[0]["manifest_source"],
        serde_json::json!({"kind": "index", "index": bucket_file.to_str().unwrap()})
    );
}

#[test]
fn install_unreachable_index_warns_and_falls_back_to_reachable_one() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("README.md", "hello");
    let head = git_in(&repo, &["rev-parse", "HEAD"]);
    let target = json_escape_path(repo.path());

    // Registered first, and unreachable: an absolute path that does not exist.
    let unreachable = workspace.path().join("does-not-exist");
    let unreachable_str = unreachable.to_str().unwrap();

    // Registered second, and describes the target with a valid manifest + ref.
    let bucket_file = workspace.path().join("bucket.json");
    fs::write(
        &bucket_file,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"1.0.0","description":"from index"}},"ref":"{head}"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace).args(["bucket", "add", unreachable_str]).assert().success();
    saucepan(&workspace)
        .args(["bucket", "add", bucket_file.to_str().unwrap()])
        .assert()
        .success();

    let output = saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "install should succeed via the reachable index");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unreachable index"), "stderr: {stderr}");

    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
    assert_eq!(idx[0]["resolved_commit"], head);
}

#[test]
fn install_reports_shadowed_index_entry_and_earlier_registration_wins() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let repo = make_git_repo("README.md", "hello");
    let head = git_in(&repo, &["rev-parse", "HEAD"]);
    let target = json_escape_path(repo.path());

    let first_bucket = workspace.path().join("first-bucket.json");
    fs::write(
        &first_bucket,
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"1.0.0","description":"first"}},"ref":"{head}"}}]"#
        ),
    )
    .unwrap();
    let second_bucket = workspace.path().join("second-bucket.json");
    fs::write(
        &second_bucket,
        format!(
            r#"[{{"name":"my-lib","version":"2.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"2.0.0","description":"second"}},"ref":"{head}"}}]"#
        ),
    )
    .unwrap();

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", first_bucket.to_str().unwrap()])
        .assert()
        .success();
    saucepan(&workspace)
        .args(["bucket", "add", second_bucket.to_str().unwrap()])
        .assert()
        .success();

    let output = saucepan(&workspace)
        .args(["install", repo.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("shadowed"), "stderr: {stderr}");

    let idx = read_index_json(&workspace);
    // The earlier-registered index (first-bucket.json) wins.
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
}

#[test]
fn bucket_add_registers_a_repository_target_index_and_it_resolves_a_manifest() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let target_repo = make_git_repo("README.md", "hello");
    let head = git_in(&target_repo, &["rev-parse", "HEAD"]);
    let target = json_escape_path(target_repo.path());

    // The index itself is a git repository (a repository target), not a
    // local bucket.json file — its working directory carries bucket.json at
    // its root, as `fetch_bucket_index` expects.
    let index_repo = TempDir::new().unwrap();
    git_in(&index_repo, &["init"]);
    git_in(&index_repo, &["config", "user.email", "test@test.com"]);
    git_in(&index_repo, &["config", "user.name", "test"]);
    fs::write(
        index_repo.path().join("bucket.json"),
        format!(
            r#"[{{"name":"my-lib","version":"1.0.0","url":"{target}","manifest":{{"name":"my-lib","version":"1.0.0","description":"from index"}},"ref":"{head}"}}]"#
        ),
    )
    .unwrap();
    git_in(&index_repo, &["add", "."]);
    git_in(&index_repo, &["commit", "-m", "init"]);

    write_config(&workspace, "[github]\nbinary = \"git\"\n");
    saucepan(&workspace)
        .args(["bucket", "add", index_repo.path().to_str().unwrap()])
        .assert()
        .success();

    saucepan(&workspace)
        .args(["install", target_repo.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(workspace.path().join("indexes").is_dir(), "index should have been cloned");
    let idx = read_index_json(&workspace);
    assert_eq!(idx[0]["sauce"]["version"], "1.0.0");
    assert_eq!(idx[0]["resolved_commit"], head);
}

// ── bucket registration: resolved_commit persistence (task 2.3) ──────────────
//
// The central-index spec requires "Saucepan SHALL record the index's
// resolved commit". Reads (search, cat bucket, the resolution chain) must
// never mutate buckets.json, so persistence happens only through explicit
// mutating commands: `bucket add --ref` (pinning resolves and records
// immediately) and `bucket refresh` (fetches and records on demand).

fn make_index_repo(bucket_json: &str) -> TempDir {
    let repo = TempDir::new().unwrap();
    git_in(&repo, &["init"]);
    git_in(&repo, &["config", "user.email", "test@test.com"]);
    git_in(&repo, &["config", "user.name", "test"]);
    fs::write(repo.path().join("bucket.json"), bucket_json).unwrap();
    git_in(&repo, &["add", "."]);
    git_in(&repo, &["commit", "-m", "init"]);
    repo
}

#[test]
fn bucket_add_without_ref_does_not_persist_a_resolved_commit() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let index_repo =
        make_index_repo(r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#);
    write_config(&workspace, "[local]\n");

    saucepan(&workspace)
        .args(["bucket", "add", index_repo.path().to_str().unwrap()])
        .assert()
        .success();

    let buckets = read_buckets_json(&workspace);
    assert!(
        buckets[0].get("resolved_commit").is_none(),
        "plain `bucket add` (no --ref) must stay offline and not resolve a commit: {buckets}"
    );
}

#[test]
fn bucket_add_with_ref_resolves_and_persists_the_commit() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let index_repo =
        make_index_repo(r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#);
    git_in(&index_repo, &["tag", "v1"]);
    let pinned_head = git_in(&index_repo, &["rev-parse", "HEAD"]);
    // Advance the index past the tag; the pin must still record v1's commit.
    fs::write(
        index_repo.path().join("bucket.json"),
        r#"[{"name":"a","version":"2.0.0","url":"https://example.com/a"}]"#,
    )
    .unwrap();
    git_in(&index_repo, &["add", "."]);
    git_in(&index_repo, &["commit", "-m", "bump"]);
    write_config(&workspace, "[local]\n");

    saucepan(&workspace)
        .args(["bucket", "add", index_repo.path().to_str().unwrap(), "--ref", "v1"])
        .assert()
        .success();

    let buckets = read_buckets_json(&workspace);
    assert_eq!(buckets[0]["reference"], "v1");
    assert_eq!(buckets[0]["resolved_commit"], pinned_head);
}

#[test]
fn bucket_refresh_resolves_and_persists_a_commit_for_a_previously_unpinned_index() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let index_repo =
        make_index_repo(r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#);
    let head = git_in(&index_repo, &["rev-parse", "HEAD"]);
    write_config(&workspace, "[local]\n");

    saucepan(&workspace)
        .args(["bucket", "add", index_repo.path().to_str().unwrap()])
        .assert()
        .success();
    assert!(read_buckets_json(&workspace)[0].get("resolved_commit").is_none());

    saucepan(&workspace)
        .args(["bucket", "refresh", index_repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("bucket refreshed"));

    let buckets = read_buckets_json(&workspace);
    assert_eq!(buckets[0]["resolved_commit"], head);
}

#[test]
fn bucket_refresh_updates_the_recorded_commit_after_the_index_advances() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let index_repo =
        make_index_repo(r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#);
    write_config(&workspace, "[local]\n");

    saucepan(&workspace)
        .args(["bucket", "add", index_repo.path().to_str().unwrap()])
        .assert()
        .success();
    saucepan(&workspace)
        .args(["bucket", "refresh", index_repo.path().to_str().unwrap()])
        .assert()
        .success();
    let first_head = read_buckets_json(&workspace)[0]["resolved_commit"].as_str().unwrap().to_string();

    fs::write(
        index_repo.path().join("bucket.json"),
        r#"[{"name":"a","version":"2.0.0","url":"https://example.com/a"}]"#,
    )
    .unwrap();
    git_in(&index_repo, &["add", "."]);
    git_in(&index_repo, &["commit", "-m", "bump"]);
    let second_head = git_in(&index_repo, &["rev-parse", "HEAD"]);
    assert_ne!(first_head, second_head);

    saucepan(&workspace)
        .args(["bucket", "refresh", index_repo.path().to_str().unwrap()])
        .assert()
        .success();

    let buckets = read_buckets_json(&workspace);
    assert_eq!(buckets[0]["resolved_commit"], second_head);
}

#[test]
fn bucket_add_with_unresolvable_ref_rolls_back_the_registration() {
    if !which_git() { return; }
    let workspace = TempDir::new().unwrap();
    let index_repo =
        make_index_repo(r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#);
    write_config(&workspace, "[local]\n");

    saucepan(&workspace)
        .args([
            "bucket",
            "add",
            index_repo.path().to_str().unwrap(),
            "--ref",
            "this-ref-does-not-exist",
        ])
        .assert()
        .failure();

    // A failed pin must not leave a half-registered, unresolved entry behind.
    saucepan(&workspace)
        .args(["bucket", "list"])
        .assert()
        .success()
        .stdout(contains("no buckets registered"));
}

#[test]
fn bucket_refresh_unregistered_url_errors() {
    let workspace = TempDir::new().unwrap();
    write_config(&workspace, "[local]\n");
    saucepan(&workspace)
        .args(["bucket", "refresh", "https://example.com/never-added.json"])
        .assert()
        .failure()
        .stderr(contains("not registered"));
}
