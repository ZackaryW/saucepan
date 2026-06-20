use anyhow::{bail, Result};
use std::path::Path;

use crate::config::Config;
use crate::error::NotFound;
use crate::index::{self, IndexEntry};
use crate::sources::git::{self, GitFetchOptions};

pub fn update(root: &Path, name: &str, config: &Config) -> Result<()> {
    let mut idx = index::load_index(root)?;
    let pos = idx
        .iter()
        .position(|e| e.name() == name)
        .ok_or_else(|| NotFound(format!("'{name}' is not installed")))?;

    let updated = match &idx[pos] {
        IndexEntry::Github { repo, .. } => {
            let gh = config
                .github
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("[github] source not enabled"))?;
            let opts = GitFetchOptions {
                binary: &gh.binary,
                token: gh.token.as_deref(),
                ssl_key: gh.ssl_key.as_deref(),
            };
            let repo = repo.clone();
            let sauce = git::fetch_sauce(&repo, &repo, &opts, root, "github", gh.manifest_name())?;
            IndexEntry::Github { repo, sauce }
        }
        IndexEntry::Customgit { url, .. } => {
            let cg = config
                .customgit
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("[customgit] source not enabled"))?;
            let opts = GitFetchOptions {
                binary: &cg.binary,
                token: cg.token.as_deref(),
                ssl_key: cg.ssl_key.as_deref(),
            };
            // dir_name is the final path component (the package name relative to base)
            let url = url.clone();
            let dir_name = url
                .rsplit('/')
                .next()
                .unwrap_or(&url);
            let sauce = git::fetch_sauce(&url, dir_name, &opts, root, "customgit", cg.manifest_name())?;
            IndexEntry::Customgit { url, sauce }
        }
        IndexEntry::Local { .. } => bail!("local sauces do not support update"),
    };

    println!("updated {} to {}", updated.sauce().name, updated.sauce().version);
    idx[pos] = updated;
    index::save_index(root, &idx)
}
