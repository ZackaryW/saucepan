use anyhow::Result;
use std::path::Path;

use crate::index::load_index;

pub fn list(root: &Path, json: bool) -> Result<()> {
    let index = load_index(root)?;
    if index.is_empty() {
        if !json {
            println!("no sauces installed");
        }
        return Ok(());
    }
    if json {
        for entry in &index {
            println!("{}", serde_json::to_string(entry)?);
        }
    } else {
        for entry in &index {
            let s = entry.sauce();
            println!("{} {} — {}", s.name, s.version, s.description);
        }
    }
    Ok(())
}
