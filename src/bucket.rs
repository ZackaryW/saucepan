use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::sources::git::{self, GitFetchOptions};

/// An entry in a bucket.json marketplace index.
///
/// `name`, `version`, and `url` are required and MUST stay non-`Option`: a
/// binary predating the introduction of `extra` must still be able to parse
/// an index produced by a newer binary, and that only holds if every field it
/// knows about keeps being present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketStub {
    pub name: String,
    pub version: String,
    pub url: String,
    /// Fields beyond the required three — e.g. a full manifest document, or
    /// arbitrary consumer data. Preserved verbatim on round-trip.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A registered bucket source (entry in .saucepan/buckets.json).
///
/// `url` is either a local path, a `file://` URL, or a repository target
/// (anything git/gh can clone: an `owner/repo` slug, a full Git URL, or a
/// local path to a repository rather than to a bucket.json file directly).
/// Which one it is is determined at fetch time by `fetch_bucket`, not stored
/// as a separate discriminant — see `fetch_bucket` for the exact rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketEntry {
    pub url: String,
    /// An explicit ref pinning which state of the index repository to read.
    /// Only meaningful when `url` is a repository target; resolved and
    /// recorded into `resolved_commit` so the index state that produced a
    /// manifest stays identifiable afterwards. Absent (and ignored) for
    /// local paths and `file://` URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// The commit the pinned (or most recently fetched) repository-target
    /// index resolved to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_commit: Option<String>,
}

pub type BucketIndex = Vec<BucketStub>;
pub type BucketRegistry = Vec<BucketEntry>;

/// Read a bucket.json from a local path, a `file://` URL, or a repository
/// target.
///
/// `file://` is always treated as local (the prefix is stripped and the
/// remainder read as a literal path). Otherwise, `url` is treated as a local
/// path if a file already exists there, and as a repository target
/// otherwise — a repository target is cloned or updated through the same
/// Git/gh path used for sauces (see `sources::git::fetch_bucket_index`), and
/// `bucket.json` is read from its root. This keeps local-path and
/// `file://` reads behaving exactly as before, while replacing the old
/// unconditional rejection of `http://`/`https://` with repository-target
/// handling — such a URL is now interpreted as a git remote to clone, not a
/// raw file to fetch over HTTP (no HTTP client is introduced).
pub fn fetch_bucket(url: &str, root: &Path, opts: &GitFetchOptions<'_>) -> Result<BucketIndex> {
    fetch_bucket_with_commit(url, root, opts).map(|(index, _)| index)
}

/// Like `fetch_bucket`, but also returns the commit a repository-target index
/// resolved to (`None` for a local path or `file://` URL, which have no
/// commit to record).
///
/// `fetch_bucket` (used by the manifest resolution chain, `search`, and
/// `cat bucket` — all reads) discards the commit so a read never mutates
/// `buckets.json`. This function exists for the commands that *do* need to
/// persist it: `bucket add` (when pinning with `--ref`) and `bucket refresh`,
/// which is where the central-index spec's "Saucepan SHALL record the
/// index's resolved commit" is actually satisfied — see
/// `commands::bucket::resolve_and_persist`.
pub fn fetch_bucket_with_commit(
    url: &str,
    root: &Path,
    opts: &GitFetchOptions<'_>,
) -> Result<(BucketIndex, Option<String>)> {
    if let Some(path) = local_bucket_path(url) {
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read bucket at {url}"))?;
        let index = serde_json::from_str(&contents).context("invalid bucket.json")?;
        return Ok((index, None));
    }
    let result = git::fetch_bucket_index(url, opts, root)?;
    Ok((result.index, Some(result.resolved_commit)))
}

/// If `url` names a local file (directly, or via `file://`), return the path
/// to read. Returns `None` when `url` should be treated as a repository
/// target instead — which includes a local path to a repository, since that
/// names a directory (or nothing yet, before the first clone), never a file.
fn local_bucket_path(url: &str) -> Option<PathBuf> {
    if let Some(stripped) = url.strip_prefix("file://") {
        return Some(PathBuf::from(stripped));
    }
    let candidate = Path::new(url);
    candidate.is_file().then(|| candidate.to_path_buf())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GitBinary;
    use tempfile::TempDir;

    /// Whether the `git` binary is available on PATH — mirrors the guard used
    /// by tests/integration.rs so these tests skip cleanly on a machine with
    /// no git installed instead of failing.
    fn which_git() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn git_in(repo: &TempDir, args: &[&str]) {
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
    }

    /// A git repository whose root carries a `bucket.json`, as a
    /// repository-target central index would.
    fn make_index_repo(bucket_json: &str) -> TempDir {
        let repo = TempDir::new().unwrap();
        git_in(&repo, &["init"]);
        git_in(&repo, &["config", "user.email", "test@test.com"]);
        git_in(&repo, &["config", "user.name", "test"]);
        std::fs::write(repo.path().join("bucket.json"), bucket_json).unwrap();
        git_in(&repo, &["add", "."]);
        git_in(&repo, &["commit", "-m", "init"]);
        repo
    }

    #[test]
    fn fetch_bucket_reads_local_path_directly() {
        let workspace = TempDir::new().unwrap();
        let bucket_file = workspace.path().join("bucket.json");
        std::fs::write(
            &bucket_file,
            r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#,
        )
        .unwrap();

        let binary = GitBinary::Git;
        let opts = GitFetchOptions { binary: &binary, token: None, ssl_key: None, reference: None };
        let index = fetch_bucket(bucket_file.to_str().unwrap(), workspace.path(), &opts).unwrap();

        assert_eq!(index.len(), 1);
        assert_eq!(index[0].name, "a");
    }

    #[test]
    fn fetch_bucket_reads_file_url() {
        let workspace = TempDir::new().unwrap();
        let bucket_file = workspace.path().join("bucket.json");
        std::fs::write(
            &bucket_file,
            r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#,
        )
        .unwrap();
        let file_url = format!("file://{}", bucket_file.to_str().unwrap());

        let binary = GitBinary::Git;
        let opts = GitFetchOptions { binary: &binary, token: None, ssl_key: None, reference: None };
        let index = fetch_bucket(&file_url, workspace.path(), &opts).unwrap();

        assert_eq!(index.len(), 1);
        assert_eq!(index[0].name, "a");
    }

    #[test]
    fn fetch_bucket_clones_repository_target_and_reads_bucket_json() {
        if !which_git() {
            return;
        }
        let index_repo = make_index_repo(
            r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#,
        );
        let workspace = TempDir::new().unwrap();
        let binary = GitBinary::Git;
        let opts = GitFetchOptions { binary: &binary, token: None, ssl_key: None, reference: None };

        let index =
            fetch_bucket(index_repo.path().to_str().unwrap(), workspace.path(), &opts).unwrap();

        assert_eq!(index.len(), 1);
        assert_eq!(index[0].name, "a");
        assert!(workspace.path().join("indexes").is_dir());
    }

    #[test]
    fn fetch_bucket_repository_target_respects_pinned_reference() {
        if !which_git() {
            return;
        }
        let index_repo = make_index_repo(
            r#"[{"name":"a","version":"1.0.0","url":"https://example.com/a"}]"#,
        );
        git_in(&index_repo, &["tag", "v1"]);
        // Advance the index repo past the tag; a pinned fetch must still see v1's content.
        std::fs::write(
            index_repo.path().join("bucket.json"),
            r#"[{"name":"a","version":"2.0.0","url":"https://example.com/a"}]"#,
        )
        .unwrap();
        git_in(&index_repo, &["add", "."]);
        git_in(&index_repo, &["commit", "-m", "bump"]);

        let workspace = TempDir::new().unwrap();
        let binary = GitBinary::Git;
        let opts =
            GitFetchOptions { binary: &binary, token: None, ssl_key: None, reference: Some("v1") };

        let index =
            fetch_bucket(index_repo.path().to_str().unwrap(), workspace.path(), &opts).unwrap();

        assert_eq!(index[0].version, "1.0.0");
    }

    /// Mirrors the pre-`extra` definition of `BucketStub` so a test can prove
    /// that a binary built against it still parses an index produced by the
    /// current, widened definition.
    #[derive(Debug, Deserialize)]
    struct PreChangeBucketStub {
        name: String,
        version: String,
        url: String,
    }

    #[test]
    fn entry_missing_required_field_is_rejected() {
        // `name` omitted.
        let raw = r#"{"version":"1.0.0","url":"https://example.com/a"}"#;
        assert!(serde_json::from_str::<BucketStub>(raw).is_err());

        // `version` omitted.
        let raw = r#"{"name":"a","url":"https://example.com/a"}"#;
        assert!(serde_json::from_str::<BucketStub>(raw).is_err());

        // `url` omitted.
        let raw = r#"{"name":"a","version":"1.0.0"}"#;
        assert!(serde_json::from_str::<BucketStub>(raw).is_err());
    }

    #[test]
    fn index_with_extra_fields_roundtrips_without_losing_them() {
        let raw = r#"[{
            "name":"a",
            "version":"1.0.0",
            "url":"https://example.com/a",
            "manifest": {"commands": ["build", "test"]},
            "note": "curated by zush"
        }]"#;

        let index: BucketIndex = serde_json::from_str(raw).unwrap();
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].extra.get("note").unwrap(), "curated by zush");
        assert!(index[0].extra.contains_key("manifest"));

        // Round-trip through serialization and back; extras must survive.
        let serialized = serde_json::to_string(&index).unwrap();
        let reparsed: BucketIndex = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed[0].extra.get("note").unwrap(), "curated by zush");
        assert_eq!(
            reparsed[0].extra.get("manifest").unwrap(),
            &serde_json::json!({"commands": ["build", "test"]})
        );
    }

    #[test]
    fn stub_with_extras_still_parses_as_pre_change_struct() {
        let stub = BucketStub {
            name: "a".to_string(),
            version: "1.0.0".to_string(),
            url: "https://example.com/a".to_string(),
            extra: {
                let mut m = serde_json::Map::new();
                m.insert("manifest".to_string(), serde_json::json!({"commands": []}));
                m
            },
        };
        let serialized = serde_json::to_string(&stub).unwrap();

        // An older binary, unaware of `extra`, still parses this successfully
        // and simply ignores what it does not recognize.
        let old: PreChangeBucketStub = serde_json::from_str(&serialized).unwrap();
        assert_eq!(old.name, "a");
        assert_eq!(old.version, "1.0.0");
        assert_eq!(old.url, "https://example.com/a");
    }
}
