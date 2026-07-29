use anyhow::Result;
use std::path::Path;

use crate::bucket::fetch_bucket_with_commit;
use crate::config::Config;
use crate::error::NotFound;
use crate::index::{load_registry, registry_add, registry_remove, save_registry};
use crate::sources::git::GitFetchOptions;

/// Register a bucket (local path, `file://` URL, or repository target).
///
/// `reference` pins a repository-target index to an explicit ref. When a ref
/// is given, the entry is resolved and its commit persisted immediately —
/// this is the "Pinning an index" scenario from the central-index spec
/// ("that ref is resolved and recorded"). When no ref is given, registration
/// stays a pure, local, offline operation exactly as before this change; use
/// `bucket refresh` later to fetch and record its resolved commit on demand.
pub fn add(root: &Path, url: &str, reference: Option<&str>, config: &Config) -> Result<()> {
    registry_add(root, url, reference)?;
    if reference.is_some()
        && let Err(e) = resolve_and_persist(root, url, config)
    {
        // A `--ref` is a request to pin *now*; if that ref cannot be
        // resolved (bad ref, unreachable repo), roll back the registration
        // rather than leaving a half-registered entry with no resolved
        // commit behind for a command that reported failure.
        let _ = registry_remove(root, url);
        return Err(e);
    }
    println!("bucket added: {url}");
    Ok(())
}

/// Re-fetch an already-registered bucket and persist the commit it resolved
/// to. This is the explicit, mutating counterpart to a plain read (`search`,
/// `cat bucket`): those never write to `buckets.json`, so this is where a
/// registered index's `resolved_commit` gets (re-)recorded when it was not
/// pinned at registration time, or when the underlying repository has since
/// moved and the recorded commit should be brought up to date.
///
/// A local path or `file://` URL has no commit to record; refreshing one is
/// a harmless no-op beyond confirming it still reads successfully.
pub fn refresh(root: &Path, url: &str, config: &Config) -> Result<()> {
    resolve_and_persist(root, url, config)?;
    println!("bucket refreshed: {url}");
    Ok(())
}

fn resolve_and_persist(root: &Path, url: &str, config: &Config) -> Result<()> {
    let mut reg = load_registry(root)?;
    let pos = reg
        .iter()
        .position(|e| e.url == url)
        .ok_or_else(|| NotFound(format!("bucket not registered: {url}")))?;

    let binary = config.index_binary();
    let opts = GitFetchOptions {
        binary: &binary,
        token: config.index_token(),
        ssl_key: config.index_ssl_key(),
        reference: reg[pos].reference.as_deref(),
    };
    let (_, resolved_commit) = fetch_bucket_with_commit(url, root, &opts)?;
    reg[pos].resolved_commit = resolved_commit;
    save_registry(root, &reg)
}

pub fn remove(root: &Path, url: &str) -> Result<()> {
    registry_remove(root, url)?;
    println!("bucket removed: {url}");
    Ok(())
}

pub fn list(root: &Path, json: bool) -> Result<()> {
    let registry = load_registry(root)?;
    if registry.is_empty() {
        if !json {
            println!("no buckets registered");
        }
        return Ok(());
    }
    if json {
        for entry in &registry {
            println!("{}", serde_json::to_string(entry)?);
        }
    } else {
        for entry in &registry {
            println!("{}", entry.url);
        }
    }
    Ok(())
}
