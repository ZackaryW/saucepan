use anyhow::Result;
use std::path::Path;

use crate::bucket::fetch_bucket;
use crate::error::NotFound;
use crate::index::{load_index, load_registry};

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

pub fn cat_bucket(url: &str) -> Result<()> {
    let stubs = fetch_bucket(url)?;
    println!("{}", serde_json::to_string_pretty(&stubs)?);
    Ok(())
}
