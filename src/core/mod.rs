pub mod activation;
pub mod bootstrap;
pub mod index;
pub mod launch;
pub mod policy;
pub mod source;
pub mod store;

use crate::utils::{
    lock::{LockGuard, LockMode},
    path::plain_metadata,
};
use anyhow::{Result, ensure};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone)]
pub struct Store {
    pub(crate) root: PathBuf,
    #[cfg(test)]
    pub(crate) access_observer:
        Option<std::sync::Arc<dyn Fn(crate::utils::tree::IoEvent) + Send + Sync>>,
}
impl Store {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn create(root: &Path) -> Result<Self> {
        if root.exists() {
            ensure!(
                plain_metadata(root)?.is_dir(),
                "store root is not a directory"
            );
            ensure!(
                std::fs::read_dir(root)?.next().is_none(),
                "initialization needs an empty new-format root; legacy data is not migrated"
            );
        } else {
            std::fs::create_dir_all(root)?;
        }
        let root = root.canonicalize()?;
        for part in ["source", "index", "store", "bin", ".locks", ".staging"] {
            std::fs::create_dir(root.join(part))?;
        }
        Ok(Self {
            root,
            #[cfg(test)]
            access_observer: None,
        })
    }
    pub fn open(root: &Path) -> Result<Self> {
        ensure!(plain_metadata(root)?.is_dir(), "invalid store root");
        ensure!(
            !root.join("index.json.enc").exists(),
            "legacy store; use a separate new-format root"
        );
        let root = root.canonicalize()?;
        for part in ["source", "index", "store", "bin", ".locks", ".staging"] {
            ensure!(
                plain_metadata(&root.join(part))?.is_dir(),
                "invalid store layout"
            );
        }
        let store = Self {
            root,
            #[cfg(test)]
            access_observer: None,
        };
        {
            let _guard = store.lock()?;
        }
        Ok(store)
    }
    pub(crate) fn lock(&self) -> Result<LockGuard> {
        let guard = LockGuard::acquire(
            &self.root.join(".locks/mutation"),
            LockMode::Exclusive,
            Duration::from_secs(30),
        )?;
        self.recover_locked()?;
        Ok(guard)
    }
}
