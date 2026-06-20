use anyhow::Result;
use std::path::Path;

use crate::index::{load_registry, registry_add, registry_remove};

pub fn add(root: &Path, url: &str) -> Result<()> {
    registry_add(root, url)?;
    println!("bucket added: {url}");
    Ok(())
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
