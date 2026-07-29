use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::bucket::fetch_bucket;
use crate::config::Config;
use crate::index::load_registry;
use crate::sources::git::GitFetchOptions;

pub fn search(root: &Path, filter: &str, config: &Config) -> Result<()> {
    let registry = load_registry(root)?;
    if registry.is_empty() {
        println!("no buckets registered");
        return Ok(());
    }

    // Standalone bucket fetches (no primary source in flight to inherit auth
    // from) use the dedicated `[index]` config section, defaulting to plain,
    // unauthenticated git when unconfigured. Local paths and file:// URLs —
    // the common case — never need auth and are unaffected either way.
    let binary = config.index_binary();

    let mut all_stubs: Vec<serde_json::Value> = vec![];
    for entry in &registry {
        let opts = GitFetchOptions {
            binary: &binary,
            token: config.index_token(),
            ssl_key: config.index_ssl_key(),
            reference: entry.reference.as_deref(),
        };
        match fetch_bucket(&entry.url, root, &opts) {
            Ok(stubs) => {
                for stub in stubs {
                    all_stubs.push(serde_json::to_value(&stub)?);
                }
            }
            Err(e) => eprintln!("warning: skipping bucket {}: {e}", entry.url),
        }
    }

    let json_input = serde_json::to_string(&all_stubs)?;

    let mut child = Command::new(config.jq_bin())
        .arg("-c")
        .arg(format!(".[] | select({filter})"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn jq — is it installed and on PATH?")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(json_input.as_bytes())?;
    }

    let result = child.wait_with_output()?;
    if !result.status.success() {
        bail!("jq exited with status {}", result.status);
    }

    let out = String::from_utf8_lossy(&result.stdout);
    if out.trim().is_empty() {
        println!("no matches");
    } else {
        print!("{out}");
    }
    Ok(())
}
