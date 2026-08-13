use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bucket::{self, BucketIndex};
use crate::config::GitBinary;
use crate::error::{NotFound, SourceError};
use crate::index::{self, ManifestSource};
use crate::sauce::Sauce;
use crate::utils::naming::{self, repo_dir};

pub struct GitFetchOptions<'a> {
    pub binary: &'a GitBinary,
    pub token: Option<&'a str>,
    pub ssl_key: Option<&'a str>,
    pub reference: Option<&'a str>,
}

pub struct GitFetchResult {
    pub sauce: Sauce,
    pub resolved_commit: String,
    /// Which link of the manifest resolution chain supplied `sauce`. Read by
    /// `install`/`update` and recorded onto the resulting `IndexEntry`.
    pub manifest_source: ManifestSource,
}

/// Clone or update a repo and return the parsed sauce manifest.
///
/// - `repo_url`  — the remote URL passed to git/gh (what to clone from)
/// - `dir_name`  — logical name used to derive the on-disk directory
///   (e.g. `owner/repo` for github, `package-name` for customgit)
/// - `root`      — workspace root; repo lands at `<root>/<source_subdir>/<repo_dir(dir_name)>/`
pub fn fetch_sauce(
    repo_url: &str,
    dir_name: &str,
    opts: &GitFetchOptions<'_>,
    root: &Path,
    source_subdir: &str,
    manifest: &str,
) -> Result<GitFetchResult> {
    let dest = clone_or_update(repo_url, dir_name, opts, root, source_subdir)?;

    let resolved = resolve_manifest(&dest, repo_url, opts, root, manifest)?
        .ok_or_else(|| NotFound(format!("no {manifest} found in {repo_url}")))?;

    let resolved_commit = git_output(opts, &dest, &["rev-parse", "HEAD"])?;
    Ok(GitFetchResult {
        sauce: resolved.sauce,
        resolved_commit,
        manifest_source: resolved.source,
    })
}

/// Clone or update a repository target to `<root>/<source_subdir>/<repo_dir(dir_name)>/`,
/// checking out `opts.reference` if given, otherwise pulling latest.
///
/// Extracted so the exact same clone/pull/checkout/authentication path can be
/// reused for a central index registered as a repository target
/// (`fetch_bucket_index`), per the design's constraint that a remote index is
/// fetched through the existing Git/gh machinery rather than a new HTTP client.
fn clone_or_update(
    repo_url: &str,
    dir_name: &str,
    opts: &GitFetchOptions<'_>,
    root: &Path,
    source_subdir: &str,
) -> Result<PathBuf> {
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
    } else {
        if dest.exists() {
            // Exists but not a valid git repo (partial or failed previous clone).
            // Remove the debris and re-clone so we don't end up stuck.
            std::fs::remove_dir_all(&dest)
                .with_context(|| format!("cannot clean up partial clone at {}", dest.display()))?;
        }
        clone(repo_url, opts, &dest)?;
        if let Some(reference) = opts.reference {
            fetch_and_checkout(opts, &dest, reference)?;
        }
    }

    Ok(dest)
}

// ── manifest resolution chain ────────────────────────────────────────────────

/// A manifest, and the chain link that supplied it.
struct ResolvedManifest {
    sauce: Sauce,
    source: ManifestSource,
}

/// A single link in the manifest resolution chain.
///
/// The chain is an ordered list of links (`default_chain`), not a fixed pair
/// of checks: a future link — e.g. a manifest generator, explicitly deferred
/// by the design as a further resolution step — can be appended after
/// `CentralIndex` without altering the precedence or outcome of the links
/// that exist today.
///
/// `Ok(None)` means "this link has nothing to offer for this target"; the
/// chain moves on to the next link. A link that finds a candidate but must
/// reject it (e.g. a manifest-supplying index entry with no ref) also
/// returns `Ok(None)` for that candidate — an invalid or unreachable entry
/// never fails the chain, it is only ever skipped.
enum ManifestLink {
    /// The fetched target's own root manifest. Always consulted first and
    /// always wins when present: a repository's own manifest describes its
    /// own identity, and a third party's index must never be able to
    /// override that.
    Repository,
    /// Registered central indexes, consulted only when the target carries
    /// no manifest of its own.
    ///
    /// Precedence among indexes is deterministic registration order: the
    /// earliest-registered index describing the target wins, and a later
    /// index also describing it is reported as shadowed rather than
    /// silently ignored. An index that cannot be fetched or read is warned
    /// about and skipped — it never fails an operation another link (or
    /// another index) can satisfy. An entry that supplies a manifest
    /// without a ref is rejected and the chain continues, since accepting
    /// it would let a recorded version disagree with the checked-out tree.
    CentralIndex,
}

impl ManifestLink {
    fn resolve(
        &self,
        dest: &Path,
        repo_url: &str,
        opts: &GitFetchOptions<'_>,
        root: &Path,
        manifest: &str,
    ) -> Result<Option<ResolvedManifest>> {
        match self {
            ManifestLink::Repository => {
                let manifest_path = dest.join(manifest);
                if !manifest_path.exists() {
                    return Ok(None);
                }
                let contents = std::fs::read_to_string(&manifest_path).with_context(|| {
                    format!("cannot read {manifest} from {}", manifest_path.display())
                })?;
                let sauce: Sauce = serde_json::from_str(&contents)
                    .map_err(|e| SourceError(format!("invalid {manifest}: {e}")))?;
                Ok(Some(ResolvedManifest {
                    sauce,
                    source: ManifestSource::repository(),
                }))
            }
            ManifestLink::CentralIndex => {
                let registry = match index::load_registry(root) {
                    Ok(registry) => registry,
                    Err(e) => {
                        eprintln!("warning: cannot read registered indexes: {e:#}");
                        return Ok(None);
                    }
                };

                let mut winner: Option<(String, ResolvedManifest)> = None;
                let mut shadowed_by: Vec<String> = Vec::new();

                for entry in &registry {
                    let index_opts = GitFetchOptions {
                        binary: opts.binary,
                        token: opts.token,
                        ssl_key: opts.ssl_key,
                        reference: entry.reference.as_deref(),
                    };

                    let stubs: BucketIndex =
                        match bucket::fetch_bucket(&entry.url, root, &index_opts) {
                            Ok(stubs) => stubs,
                            Err(e) => {
                                eprintln!(
                                    "warning: skipping unreachable index {}: {e:#}",
                                    entry.url
                                );
                                continue;
                            }
                        };

                    for stub in &stubs {
                        // Compared normalized, so a stub naming the same
                        // repository as the target still matches when the two
                        // spellings differ trivially (`.git` suffix, trailing
                        // separator, `\` vs `/`, or another of the equivalent
                        // GitHub forms). Both strings stay as-given for
                        // storage and display — see `naming::normalize_target`.
                        if naming::normalize_target(&stub.url) != naming::normalize_target(repo_url)
                        {
                            continue;
                        }

                        let Some(manifest_value) = stub.extra.get("manifest") else {
                            // This stub describes the target but supplies no manifest —
                            // nothing for this link to offer from this entry.
                            continue;
                        };
                        let Some(entry_ref) = stub.extra.get("ref").and_then(|v| v.as_str()) else {
                            eprintln!(
                                "warning: index {} entry for {repo_url} supplies a manifest \
                                 without a ref; rejecting entry",
                                entry.url
                            );
                            continue;
                        };

                        let sauce: Sauce = match serde_json::from_value(manifest_value.clone()) {
                            Ok(sauce) => sauce,
                            Err(e) => {
                                eprintln!(
                                    "warning: index {} entry for {repo_url} has an invalid \
                                     manifest: {e}; rejecting entry",
                                    entry.url
                                );
                                continue;
                            }
                        };

                        if winner.is_some() {
                            shadowed_by.push(entry.url.clone());
                            continue;
                        }

                        // Check out the entry-supplied ref on the target's own
                        // checkout so the recorded version can never disagree with
                        // the working tree it came from.
                        match fetch_and_checkout(opts, dest, entry_ref) {
                            Ok(()) => {
                                winner = Some((
                                    entry.url.clone(),
                                    ResolvedManifest {
                                        sauce,
                                        source: ManifestSource::index(entry.url.clone()),
                                    },
                                ));
                            }
                            Err(e) => {
                                eprintln!(
                                    "warning: index {} entry for {repo_url} points to ref \
                                     \"{entry_ref}\" which could not be checked out: {e:#}; \
                                     rejecting entry",
                                    entry.url
                                );
                            }
                        }
                    }
                }

                match winner {
                    Some((winning_url, resolved)) => {
                        for other in shadowed_by {
                            eprintln!(
                                "warning: index {other} also describes {repo_url}; shadowed by \
                                 {winning_url}"
                            );
                        }
                        Ok(Some(resolved))
                    }
                    None => Ok(None),
                }
            }
        }
    }
}

/// The manifest resolution chain, in precedence order.
fn default_chain() -> Vec<ManifestLink> {
    vec![ManifestLink::Repository, ManifestLink::CentralIndex]
}

/// Walk the chain and return the first link's manifest, if any.
fn resolve_manifest(
    dest: &Path,
    repo_url: &str,
    opts: &GitFetchOptions<'_>,
    root: &Path,
    manifest: &str,
) -> Result<Option<ResolvedManifest>> {
    for link in default_chain() {
        if let Some(resolved) = link.resolve(dest, repo_url, opts, root, manifest)? {
            return Ok(Some(resolved));
        }
    }
    Ok(None)
}

// ── central index fetch ──────────────────────────────────────────────────────

/// The file read from a repository-target index once it has been cloned or
/// pulled: the bucket.json is expected at the root of the index repository.
const INDEX_BUCKET_FILE: &str = "bucket.json";

/// Result of fetching a repository-target central index: its parsed stubs,
/// and the commit the fetched state resolved to — so the index state that
/// produced a manifest stays identifiable afterwards.
pub struct BucketFetchResult {
    pub index: BucketIndex,
    /// The commit the index repository resolved to. `bucket::fetch_bucket`
    /// discards this (reads never mutate `buckets.json`);
    /// `bucket::fetch_bucket_with_commit` surfaces it so `bucket add`/
    /// `bucket refresh` can persist it onto `BucketEntry::resolved_commit`.
    pub resolved_commit: String,
}

/// Fetch a central index registered as a repository target, through the same
/// clone/pull/checkout/authentication path used for sauces. `target` is
/// cloned or updated at `<root>/indexes/<repo_dir(target)>/`, and
/// `bucket.json` is read from its root.
pub fn fetch_bucket_index(
    target: &str,
    opts: &GitFetchOptions<'_>,
    root: &Path,
) -> Result<BucketFetchResult> {
    let dest = clone_or_update(target, target, opts, root, "indexes")?;
    let bucket_path = dest.join(INDEX_BUCKET_FILE);
    let contents = std::fs::read_to_string(&bucket_path).with_context(|| {
        format!(
            "cannot read {INDEX_BUCKET_FILE} from {}",
            bucket_path.display()
        )
    })?;
    let index: BucketIndex = serde_json::from_str(&contents).context("invalid bucket.json")?;
    let resolved_commit = git_output(opts, &dest, &["rev-parse", "HEAD"])?;
    Ok(BucketFetchResult {
        index,
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

/// Refresh an unpinned checkout regardless of its original clone binary.
/// After a `gh repo clone`, the credential helper is already configured so
/// plain Git works inside the repo. A detached checkout can occur when a
/// central index supplied the previous manifest ref; fetch its refs without
/// merging so the current resolution chain can select the next ref.
fn pull(opts: &GitFetchOptions<'_>, dest: &Path) -> Result<()> {
    let mut cmd = git_command(opts);
    cmd.current_dir(dest);
    if try_git_output(opts, dest, &["symbolic-ref", "-q", "HEAD"]).is_none() {
        cmd.args(["fetch", "origin", "--tags", "--prune"]);
    } else {
        cmd.arg("pull");
    }
    run(cmd)
}

fn fetch_and_checkout(opts: &GitFetchOptions<'_>, dest: &Path, reference: &str) -> Result<()> {
    let mut fetch = git_command(opts);
    fetch
        .current_dir(dest)
        .args(["fetch", "origin", "--tags", "--prune"]);
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
    checkout
        .current_dir(dest)
        .args(["checkout", "--detach", &resolved]);
    run(checkout)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn git_command(opts: &GitFetchOptions<'_>) -> Command {
    let mut cmd = Command::new("git");
    if let Some(key) = opts.ssl_key {
        cmd.env("GIT_SSL_KEY", key);
    }
    if opts.binary == &GitBinary::Gh
        && let Some(token) = opts.token
    {
        cmd.env("GITHUB_TOKEN", token);
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
    use super::{GitFetchOptions, git_command, github_clone_target};
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
        assert_eq!(
            github_clone_target("owner/repo", &GitBinary::Gh),
            "owner/repo"
        );
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
