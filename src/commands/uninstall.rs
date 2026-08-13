use anyhow::{Context, Result, bail};
use std::path::{Component, Path};

use crate::error::NotFound;
use crate::index::{self, IndexEntry};

pub fn uninstall(root: &Path, name: &str) -> Result<()> {
    let mut entries = index::load_index(root)?;
    let pos = entries
        .iter()
        .position(|entry| entry.name() == name)
        .ok_or_else(|| NotFound(format!("'{name}' is not installed")))?;

    let entry = entries[pos].clone();
    let managed_base = match &entry {
        IndexEntry::Local { .. } => None,
        IndexEntry::Github { .. } => Some(root.join("github")),
        IndexEntry::Customgit { .. } => Some(root.join("customgit")),
    };

    if let Some(base) = managed_base {
        let checkout = entry.artifact_path(root);
        ensure_managed_path(&base, &checkout)?;
        if checkout.exists() {
            std::fs::remove_dir_all(&checkout).with_context(|| {
                format!("cannot remove managed checkout {}", checkout.display())
            })?;
        }
    }

    entries.remove(pos);
    index::save_index(root, &entries)?;
    println!("uninstalled {name}");
    Ok(())
}

fn ensure_managed_path(base: &Path, candidate: &Path) -> Result<()> {
    let relative = candidate.strip_prefix(base).ok();
    let is_single_child = relative.is_some_and(|path| {
        let mut components = path.components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    });

    if is_single_child {
        Ok(())
    } else {
        bail!(
            "refusing to remove path outside managed directory: {}",
            candidate.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_managed_path;
    use tempfile::TempDir;

    #[test]
    fn managed_path_accepts_child_of_source_directory() {
        let root = TempDir::new().unwrap();
        let base = root.path().join("github");
        let checkout = base.join("owner--repo");

        assert!(ensure_managed_path(&base, &checkout).is_ok());
    }

    #[test]
    fn managed_path_rejects_escape_from_source_directory() {
        let root = TempDir::new().unwrap();
        let base = root.path().join("github");
        let escaped = root.path().join("outside");

        assert!(ensure_managed_path(&base, &escaped).is_err());
    }

    #[test]
    fn managed_path_rejects_parent_component_beneath_source_directory() {
        let root = TempDir::new().unwrap();
        let base = root.path().join("customgit");
        let escaped = base.join("..");

        assert!(ensure_managed_path(&base, &escaped).is_err());
    }
}
