use anyhow::Result;
use std::path::Path;

use crate::config::Config;
use crate::error::{ConfigError, NotFound, SourceError};
use crate::index::{self, IndexEntry};
use crate::sources::git::{self, GitFetchOptions};

pub fn install(
    root: &Path,
    name: &str,
    reference: Option<&str>,
    config: &Config,
) -> Result<()> {
    if !config.local_enabled() && !config.github_enabled() && !config.customgit_enabled() {
        return Err(ConfigError("no sources enabled in saucepan.toml".to_string()).into());
    }

    // Load the index once; reuse for both the existence check and the save.
    let mut idx = index::load_index(root)?;

    if config.local_enabled() {
        if let Some(entry) = idx.iter().find(|e| e.name() == name) {
            println!("already installed: {} {}", entry.sauce().name, entry.sauce().version);
            return Ok(());
        }
    }

    let mut failures = Vec::new();
    let mut had_source_error = false;

    if let Some(gh) = &config.github {
        let opts = GitFetchOptions {
            binary: &gh.binary,
            token: gh.token.as_deref(),
            ssl_key: gh.ssl_key.as_deref(),
            reference,
        };
        match git::fetch_sauce(name, name, &opts, root, "github", gh.manifest_name()) {
            Ok(fetched) => {
                index::upsert(&mut idx, IndexEntry::Github {
                    repo: name.to_string(),
                    reference: reference.map(str::to_string),
                    resolved_commit: Some(fetched.resolved_commit),
                    manifest_source: fetched.manifest_source,
                    sauce: fetched.sauce,
                })?;
                index::save_index(root, &idx)?;
                println!("installed {name} from github");
                return Ok(());
            }
            Err(e) => {
                had_source_error |= e.downcast_ref::<SourceError>().is_some();
                failures.push(format!("github source: {e:#}"));
            }
        }
    }

    if let Some(cg) = &config.customgit {
        let opts = GitFetchOptions {
            binary: &cg.binary,
            token: cg.token.as_deref(),
            ssl_key: cg.ssl_key.as_deref(),
            reference: None,
        };
        let repo_url = format!("{}/{}", cg.url.trim_end_matches('/'), name);
        match git::fetch_sauce(&repo_url, name, &opts, root, "customgit", cg.manifest_name()) {
            Ok(fetched) => {
                index::upsert(
                    &mut idx,
                    IndexEntry::Customgit {
                        url: repo_url,
                        manifest_source: fetched.manifest_source,
                        sauce: fetched.sauce,
                    },
                )?;
                index::save_index(root, &idx)?;
                println!("installed {name} from customgit");
                return Ok(());
            }
            Err(e) => {
                had_source_error |= e.downcast_ref::<SourceError>().is_some();
                failures.push(format!("customgit source: {e:#}"));
            }
        }
    }

    let context = if failures.is_empty() {
        String::new()
    } else {
        format!(": {}", failures.join("; "))
    };
    let message = format!("could not install '{name}' from any enabled source{context}");
    if had_source_error {
        Err(SourceError(message).into())
    } else {
        Err(NotFound(message).into())
    }
}
