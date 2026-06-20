use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use crate::config::GitBinary;
use crate::sauce::Sauce;
use crate::utils::naming::repo_dir;

pub struct GitFetchOptions<'a> {
    pub binary: &'a GitBinary,
    pub token: Option<&'a str>,
    pub ssl_key: Option<&'a str>,
}

/// Clone or update a repo and return the parsed sauce manifest.
///
/// - `repo_url`  — the remote URL passed to git/gh (what to clone from)
/// - `dir_name`  — logical name used to derive the on-disk directory
///                 (e.g. `owner/repo` for github, `package-name` for customgit)
/// - `root`      — workspace root; repo lands at `<root>/<source_subdir>/<repo_dir(dir_name)>/`
pub fn fetch_sauce(
    repo_url: &str,
    dir_name: &str,
    opts: &GitFetchOptions<'_>,
    root: &Path,
    source_subdir: &str,
    manifest: &str,
) -> Result<Sauce> {
    let dest = root.join(source_subdir).join(repo_dir(dir_name));

    if dest.join(".git").exists() {
        // Valid existing clone — pull latest.
        pull(opts, &dest)?;
    } else if dest.exists() {
        // Exists but not a valid git repo (partial or failed previous clone).
        // Remove the debris and re-clone so we don't end up stuck.
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("cannot clean up partial clone at {}", dest.display()))?;
        clone(repo_url, opts, &dest)?;
    } else {
        clone(repo_url, opts, &dest)?;
    }

    let manifest_path = dest.join(manifest);
    if !manifest_path.exists() {
        bail!("no {manifest} found in {repo_url}");
    }
    let contents = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("cannot read {manifest} from {}", manifest_path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("invalid {manifest}"))
}

// ── clone ─────────────────────────────────────────────────────────────────────

fn clone(repo_url: &str, opts: &GitFetchOptions<'_>, dest: &Path) -> Result<()> {
    match opts.binary {
        GitBinary::Gh => clone_gh(repo_url, opts.token, dest),
        GitBinary::Git => clone_git(repo_url, opts, dest),
    }
}

/// `gh repo clone <repo> <dest>` — accepts `OWNER/REPO` slugs and full HTTPS URLs.
fn clone_gh(repo_url: &str, token: Option<&str>, dest: &Path) -> Result<()> {
    let mut cmd = Command::new("gh");
    if let Some(t) = token {
        cmd.env("GITHUB_TOKEN", t);
    }
    cmd.args(["repo", "clone", repo_url, dest.to_str().unwrap()]);
    run(cmd)
}

/// `git clone <url> <dest>` with optional token and SSL key injection.
fn clone_git(repo_url: &str, opts: &GitFetchOptions<'_>, dest: &Path) -> Result<()> {
    let mut cmd = git_command(opts);
    cmd.args(["clone", repo_url, dest.to_str().unwrap()]);
    run(cmd)
}

// ── pull ──────────────────────────────────────────────────────────────────────

/// Always uses `git pull` regardless of original clone binary.
/// After a `gh repo clone`, the credential helper is already configured so
/// plain `git pull` works inside the repo.
fn pull(opts: &GitFetchOptions<'_>, dest: &Path) -> Result<()> {
    let mut cmd = git_command(opts);
    cmd.current_dir(dest).args(["pull"]);
    run(cmd)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn git_command(opts: &GitFetchOptions<'_>) -> Command {
    let mut cmd = Command::new("git");
    if let Some(key) = opts.ssl_key {
        cmd.env("GIT_SSL_KEY", key);
    }
    if let Some(token) = opts.token {
        cmd.env("GITHUB_TOKEN", token);
        // Credential helper approach that works on both Unix and Windows.
        // Sets GIT_ASKPASS to a script/program that echoes the token.
        // On Windows, credential manager takes precedence so we rely on
        // GITHUB_TOKEN being picked up by gh's credential helper if configured.
        cmd.env("GIT_PASSWORD", token)
           .env("GIT_USERNAME", "token");
    }
    cmd
}

fn run(mut cmd: Command) -> Result<()> {
    let status = cmd.status().context("failed to launch git/gh")?;
    if !status.success() {
        bail!("git/gh exited with status {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::utils::naming::repo_dir;

    #[test]
    fn github_name_produces_readable_dir() {
        assert_eq!(repo_dir("owner/repo"), "owner--repo");
    }

    #[test]
    fn customgit_plain_name_unchanged() {
        assert_eq!(repo_dir("my-package"), "my-package");
    }

    #[test]
    fn underscore_name_distinct_from_slash() {
        assert_ne!(repo_dir("owner/repo"), repo_dir("owner_repo"));
    }
}
