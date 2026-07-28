use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

mod utils;
use utils::git::{commit_manifest, default_sauce_json, git_in, make_git_repo, which_git};
use utils::index::{read_index_json, read_index_text};

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
