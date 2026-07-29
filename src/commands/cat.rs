use anyhow::Result;
use std::path::Path;

use crate::bucket::fetch_bucket;
use crate::config::Config;
use crate::error::NotFound;
use crate::index::{load_index, load_registry};
use crate::sources::git::GitFetchOptions;

pub fn cat_index(root: &Path) -> Result<()> {
    let index = load_index(root)?;
    println!("{}", serde_json::to_string_pretty(&index)?);
    Ok(())
}

pub fn cat_buckets(root: &Path) -> Result<()> {
    let registry = load_registry(root)?;
    println!("{}", serde_json::to_string_pretty(&registry)?);
    Ok(())
}

pub fn cat_sauce(root: &Path, name: &str) -> Result<()> {
    let index = load_index(root)?;
    let entry = index
        .iter()
        .find(|e| e.name() == name)
        .ok_or_else(|| NotFound(format!("sauce '{name}' is not installed")))?;
    println!("{}", serde_json::to_string_pretty(entry)?);
    Ok(())
}

pub fn cat_bucket(root: &Path, url: &str, config: &Config) -> Result<()> {
    // Standalone fetch (no primary source in flight to inherit auth from):
    // use the dedicated `[index]` config section, defaulting to plain,
    // unauthenticated git when unconfigured. If `url` matches a registered
    // bucket, reuse its pinned reference so an ad hoc `cat bucket` of a
    // pinned index reads the same state `search`/install resolution would.
    let registry = load_registry(root)?;
    let reference = registry.iter().find(|e| e.url == url).and_then(|e| e.reference.as_deref());
    let binary = config.index_binary();
    let opts = GitFetchOptions {
        binary: &binary,
        token: config.index_token(),
        ssl_key: config.index_ssl_key(),
        reference,
    };
    let stubs = fetch_bucket(url, root, &opts)?;
    println!("{}", serde_json::to_string_pretty(&stubs)?);
    Ok(())
}
