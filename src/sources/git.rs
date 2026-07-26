use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::config::GitBinary;
use crate::error::{NotFound, SourceError};
use crate::sauce::Sauce;
use crate::utils::naming::repo_dir;

pub struct GitFetchOptions<'a> {
    pub binary: &'a GitBinary,
    pub token: Option<&'a str>,
    pub ssl_key: Option<&'a str>,
    pub reference: Option<&'a str>,
}

pub struct GitFetchResult {
    pub sauce: Sauce,
    pub resolved_commit: String,
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
) -> Result<GitFetchResult> {
    if opts.binary == &GitBinary::Git && opts.token.is_some() {
        eprintln!(
            "warning: token is not injected for binary = \"git\"; using native Git credentials"
        );
    }
    let source_path = Path::new(repo_url);
    if source_path.is_absolute() && !source_path.exists() {
        return Err(NotFound(format!("repository not found: {repo_url}")).into());
    }
    let dest = root.join(source_subdir).join(repo_dir(dir_name));

    if dest.join(".git").exists() {
        if let Some(reference) = opts.reference {
            fetch_and_checkout(opts, &dest, reference)?;
        } else {
            // Valid existing clone — pull latest.
            pull(opts, &dest)?;
        }
    } else if dest.exists() {
        // Exists but not a valid git repo (partial or failed previous clone).
        // Remove the debris and re-clone so we don't end up stuck.
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("cannot clean up partial clone at {}", dest.display()))?;
        clone(repo_url, opts, &dest)?;
        if let Some(reference) = opts.reference {
            fetch_and_checkout(opts, &dest, reference)?;
        }
    } else {
        clone(repo_url, opts, &dest)?;
        if let Some(reference) = opts.reference {
            fetch_and_checkout(opts, &dest, reference)?;
        }
    }

    let manifest_path = dest.join(manifest);
    if !manifest_path.exists() {
        return Err(NotFound(format!("no {manifest} found in {repo_url}")).into());
    }
    let contents = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("cannot read {manifest} from {}", manifest_path.display()))?;
    let sauce = serde_json::from_str(&contents)
        .map_err(|e| SourceError(format!("invalid {manifest}: {e}")))?;
    let resolved_commit = git_output(opts, &dest, &["rev-parse", "HEAD"])?;
    Ok(GitFetchResult {
        sauce,
        resolved_commit,
    })
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
    let target = github_clone_target(repo_url, opts.binary);
    cmd.arg("clone").arg(target).arg(dest);
    run(cmd)
}

fn github_clone_target(target: &str, binary: &GitBinary) -> String {
    if binary == &GitBinary::Git && is_strict_github_slug(target) {
        format!("https://github.com/{target}.git")
    } else {
        target.to_string()
    }
}

fn is_strict_github_slug(target: &str) -> bool {
    if target.contains([':', '\\']) {
        return false;
    }
    let mut parts = target.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(repo) = parts.next() else {
        return false;
    };
    !owner.is_empty()
        && !repo.is_empty()
        && owner != "."
        && owner != ".."
        && repo != "."
        && repo != ".."
        && parts.next().is_none()
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

fn fetch_and_checkout(opts: &GitFetchOptions<'_>, dest: &Path, reference: &str) -> Result<()> {
    let mut fetch = git_command(opts);
    fetch.current_dir(dest).args(["fetch", "origin", "--tags", "--prune"]);
    run(fetch)?;

    let candidates = [
        format!("refs/remotes/origin/{reference}^{{commit}}"),
        format!("refs/tags/{reference}^{{commit}}"),
        format!("{reference}^{{commit}}"),
    ];
    let resolved = candidates
        .iter()
        .find_map(|candidate| try_git_output(opts, dest, &["rev-parse", "--verify", candidate]))
        .ok_or_else(|| NotFound(format!("git ref not found: {reference}")))?;

    let mut checkout = git_command(opts);
    checkout.current_dir(dest).args(["checkout", "--detach", &resolved]);
    run(checkout)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn git_command(opts: &GitFetchOptions<'_>) -> Command {
    let mut cmd = Command::new("git");
    if let Some(key) = opts.ssl_key {
        cmd.env("GIT_SSL_KEY", key);
    }
    if opts.binary == &GitBinary::Gh {
        if let Some(token) = opts.token {
            cmd.env("GITHUB_TOKEN", token);
        }
    }
    cmd
}

fn run(mut cmd: Command) -> Result<()> {
    let status = cmd
        .status()
        .map_err(|e| SourceError(format!("failed to launch git/gh: {e}")))?;
    if !status.success() {
        return Err(SourceError(format!("git/gh exited with status {status}")).into());
    }
    Ok(())
}

fn git_output(opts: &GitFetchOptions<'_>, dest: &Path, args: &[&str]) -> Result<String> {
    try_git_output(opts, dest, args)
        .ok_or_else(|| SourceError(format!("git command failed: git {}", args.join(" "))).into())
}

fn try_git_output(opts: &GitFetchOptions<'_>, dest: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = git_command(opts);
    let output = cmd.current_dir(dest).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::{git_command, github_clone_target, GitFetchOptions};
    use crate::config::GitBinary;
    use crate::utils::naming::repo_dir;
    use std::collections::HashMap;

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

    #[test]
    fn raw_git_normalizes_strict_github_slug() {
        assert_eq!(
            github_clone_target("owner/repo", &GitBinary::Git),
            "https://github.com/owner/repo.git"
        );
    }

    #[test]
    fn gh_keeps_strict_github_slug() {
        assert_eq!(github_clone_target("owner/repo", &GitBinary::Gh), "owner/repo");
    }

    #[test]
    fn raw_git_passes_explicit_targets_through() {
        for target in [
            "https://github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "C:\\repos\\repo",
            "/repos/repo",
            "owner/team/repo",
        ] {
            assert_eq!(github_clone_target(target, &GitBinary::Git), target);
        }
    }

    #[test]
    fn raw_git_command_does_not_inject_token_credentials() {
        let opts = GitFetchOptions {
            binary: &GitBinary::Git,
            token: Some("secret"),
            ssl_key: None,
            reference: None,
        };
        let command = git_command(&opts);
        let envs: HashMap<_, _> = command.get_envs().collect();

        assert!(!envs.contains_key(std::ffi::OsStr::new("GITHUB_TOKEN")));
        assert!(!envs.contains_key(std::ffi::OsStr::new("GIT_USERNAME")));
        assert!(!envs.contains_key(std::ffi::OsStr::new("GIT_PASSWORD")));
    }

    #[test]
    fn gh_backed_git_command_inherits_github_token() {
        let opts = GitFetchOptions {
            binary: &GitBinary::Gh,
            token: Some("secret"),
            ssl_key: None,
            reference: None,
        };
        let command = git_command(&opts);
        let envs: HashMap<_, _> = command.get_envs().collect();

        assert_eq!(
            envs.get(std::ffi::OsStr::new("GITHUB_TOKEN"))
                .and_then(|value| *value),
            Some(std::ffi::OsStr::new("secret"))
        );
    }
}
