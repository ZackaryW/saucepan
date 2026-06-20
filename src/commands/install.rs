use anyhow::{bail, Result};
use std::path::Path;

use crate::config::Config;
use crate::error::NotFound;
use crate::index::{self, IndexEntry};
use crate::sources::git::{self, GitFetchOptions};

pub fn install(root: &Path, name: &str, config: &Config) -> Result<()> {
    if !config.local_enabled() && !config.github_enabled() && !config.customgit_enabled() {
        bail!("no sources enabled in saucepan.toml");
    }

    // Load the index once; reuse for both the existence check and the save.
    let mut idx = index::load_index(root)?;

    if config.local_enabled() {
        if let Some(entry) = idx.iter().find(|e| e.name() == name) {
            println!("already installed: {} {}", entry.sauce().name, entry.sauce().version);
            return Ok(());
        }
    }

    if let Some(gh) = &config.github {
        let opts = GitFetchOptions {
            binary: &gh.binary,
            token: gh.token.as_deref(),
            ssl_key: gh.ssl_key.as_deref(),
        };
        match git::fetch_sauce(name, name, &opts, root, "github", gh.manifest_name()) {
            Ok(sauce) => {
                index::upsert(&mut idx, IndexEntry::Github { repo: name.to_string(), sauce })?;
                index::save_index(root, &idx)?;
                println!("installed {name} from github");
                return Ok(());
            }
            Err(e) => eprintln!("github source: {e}"),
        }
    }

    if let Some(cg) = &config.customgit {
        let opts = GitFetchOptions {
            binary: &cg.binary,
            token: cg.token.as_deref(),
            ssl_key: cg.ssl_key.as_deref(),
        };
        let repo_url = format!("{}/{}", cg.url.trim_end_matches('/'), name);
        match git::fetch_sauce(&repo_url, name, &opts, root, "customgit", cg.manifest_name()) {
            Ok(sauce) => {
                index::upsert(&mut idx, IndexEntry::Customgit { url: repo_url, sauce })?;
                index::save_index(root, &idx)?;
                println!("installed {name} from customgit");
                return Ok(());
            }
            Err(e) => eprintln!("customgit source: {e}"),
        }
    }

    Err(NotFound(format!("could not install '{name}' from any enabled source")).into())
}
