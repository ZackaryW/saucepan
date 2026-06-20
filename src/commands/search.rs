use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::bucket::fetch_bucket;
use crate::config::Config;
use crate::index::load_registry;

pub fn search(root: &Path, filter: &str, config: &Config) -> Result<()> {
    let registry = load_registry(root)?;
    if registry.is_empty() {
        println!("no buckets registered");
        return Ok(());
    }

    let mut all_stubs: Vec<serde_json::Value> = vec![];
    for entry in &registry {
        match fetch_bucket(&entry.url) {
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
