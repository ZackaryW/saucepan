use anyhow::Result;
use std::path::Path;

use crate::error::NotFound;
use crate::index;

pub fn path(root: &Path, name: &str) -> Result<()> {
    let idx = index::load_index(root)?;
    let entry = idx
        .iter()
        .find(|e| e.name() == name)
        .ok_or_else(|| NotFound(format!("'{name}' is not installed")))?;
    println!("{}", entry.artifact_path(root).display());
    Ok(())
}
