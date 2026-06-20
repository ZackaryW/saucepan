use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// An entry in a bucket.json marketplace index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketStub {
    pub name: String,
    pub version: String,
    pub url: String,
}

/// A registered bucket source (entry in .saucepan/buckets.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketEntry {
    pub url: String,
}

pub type BucketIndex = Vec<BucketStub>;
pub type BucketRegistry = Vec<BucketEntry>;

/// Read a bucket.json from a local path or file:// URL.
pub fn fetch_bucket(url: &str) -> Result<BucketIndex> {
    if url.starts_with("http://") || url.starts_with("https://") {
        bail!("network fetch not yet supported; use a local file:// URL or local path");
    }
    let path = url.strip_prefix("file://").unwrap_or(url);
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read bucket at {url}"))?;
    serde_json::from_str(&contents).context("invalid bucket.json")
}
