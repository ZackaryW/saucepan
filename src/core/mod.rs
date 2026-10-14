pub mod activation;
pub mod bootstrap;
pub mod claim;
pub(crate) mod consumer;
pub mod group;
pub mod index;
pub mod launch;
pub mod lease;
pub mod lifecycle;
pub mod pin;
pub mod policy;
pub mod query;
pub mod retrieval;
pub mod shims;
pub mod source;
pub mod store;

use crate::utils::{
    lock::{LockGuard, LockMode},
    path::plain_metadata,
};
use anyhow::{Result, ensure};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::Duration,
};

thread_local! {
    // Public store operations call other public readers. Share an already-held
    // native lock only on this thread and only for this canonical root. Rc makes
    // guards !Send and retains the OS lock even if nested guards drop out of order.
    static HELD_LOCKS: RefCell<BTreeMap<PathBuf, Weak<LockGuard>>> = const { RefCell::new(BTreeMap::new()) };
}
pub(crate) struct StoreGuard {
    root: PathBuf,
    native: Rc<LockGuard>,
}
impl Drop for StoreGuard {
    fn drop(&mut self) {
        if Rc::strong_count(&self.native) == 1 {
            HELD_LOCKS.with(|locks| {
                locks.borrow_mut().remove(&self.root);
            });
        }
    }
}

#[derive(Clone)]
pub struct Store {
    pub(crate) root: PathBuf,
    pub(crate) metadata: claim::persistence::Metadata,
    pub(crate) session: Option<claim::session::SessionIdentity>,
    pub(crate) metadata_only: bool,
    pub(crate) policy_fence: Option<policy::fence::PolicyFence>,
    #[cfg(test)]
    pub(crate) before_claim_commit: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    #[cfg(test)]
    pub(crate) access_observer:
        Option<std::sync::Arc<dyn Fn(crate::utils::tree::IoEvent) + Send + Sync>>,
}

#[cfg(test)]
static FIXTURE_KEYS: std::sync::LazyLock<claim::security::MemoryKeys> =
    std::sync::LazyLock::new(Default::default);
#[cfg(test)]
impl Store {
    pub(crate) fn create_fixture(root: &Path) -> Result<Self> {
        Self::create_with_keys(root, &*FIXTURE_KEYS)
    }
    pub(crate) fn open_fixture(root: &Path) -> Result<Self> {
        Self::open_with_keys(root, &*FIXTURE_KEYS)
    }
    pub(crate) fn interrupt_commit(&self, journal: index::Journal, boundary: usize) {
        let mut current = 0;
        let result = self.commit_observed(journal, |_| {
            let position = current;
            current += 1;
            if position == boundary {
                Err(crate::error::Error::new(
                    crate::error::ErrorCode::IoError,
                    "injected crash",
                ))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "publication boundary was not exercised");
    }
}
impl Store {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn create(root: &Path) -> Result<Self> {
        Self::create_with_integrity(root, crate::models::claim::security::IntegrityMode::Basic)
    }
    pub fn create_with_integrity(
        root: &Path,
        mode: crate::models::claim::security::IntegrityMode,
    ) -> Result<Self> {
        Self::create_with_integrity_and_keys(root, mode, &claim::security::NativeKeyring)
    }
    pub fn create_with_keys(root: &Path, keys: &dyn claim::security::KeyProvider) -> Result<Self> {
        Self::create_with_integrity_and_keys(
            root,
            crate::models::claim::security::IntegrityMode::Keyring,
            keys,
        )
    }
    /// Explicit integrity selection; basic mode never invokes the key provider.
    pub fn create_with_integrity_and_keys(
        root: &Path,
        mode: crate::models::claim::security::IntegrityMode,
        keys: &dyn claim::security::KeyProvider,
    ) -> Result<Self> {
        claim::persistence::Metadata::check_initialization_contract(root)?;
        if root.exists() {
            ensure!(
                plain_metadata(root)?.is_dir(),
                "store root is not a directory"
            );
            ensure!(
                std::fs::read_dir(root)?.next().is_none()
                    || root.join(".staging/initializing.json").is_file(),
                crate::error::Error::new(
                    crate::error::ErrorCode::UnsupportedStore,
                    "initialization needs an empty root or an interrupted initialization; legacy data is not migrated"
                )
            );
            for entry in std::fs::read_dir(root)? {
                let entry = entry?;
                ensure!(
                    [
                        "source",
                        "index",
                        "claims",
                        "policies",
                        "store",
                        "bin",
                        ".locks",
                        ".staging",
                        "root.json"
                    ]
                    .iter()
                    .any(|name| entry.file_name() == *name),
                    crate::error::Error::new(
                        crate::error::ErrorCode::UnsupportedStore,
                        "unknown files in initialization root"
                    )
                );
            }
        } else {
            std::fs::create_dir_all(root)?;
        }
        let root = root.canonicalize()?;
        for part in [
            "source", "index", "claims", "policies", "store", "bin", ".locks", ".staging",
        ] {
            let path = crate::utils::path::beneath(&root, part)?;
            std::fs::create_dir_all(path)?;
        }
        let _initialization = LockGuard::acquire(
            &root.join(".locks/mutation"),
            LockMode::Exclusive,
            Duration::from_secs(30),
        )?;
        Ok(Self {
            policy_fence: None,
            metadata_only: false,
            session: None,
            metadata: claim::persistence::Metadata::initialize_with_mode(&root, mode, keys)?,
            root,
            #[cfg(test)]
            before_claim_commit: None,
            #[cfg(test)]
            access_observer: None,
        })
    }
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_with_keys(root, &claim::security::NativeKeyring)
    }
    pub fn integrity_mode(&self) -> crate::models::claim::security::IntegrityMode {
        self.metadata.authenticator().mode()
    }
    pub fn open_with_keys(root: &Path, keys: &dyn claim::security::KeyProvider) -> Result<Self> {
        Self::open_mode(root, keys, false)
    }
    pub(crate) fn open_metadata(
        root: &Path,
        keys: &dyn claim::security::KeyProvider,
    ) -> Result<Self> {
        Self::open_mode(root, keys, true)
    }
    fn open_mode(
        root: &Path,
        keys: &dyn claim::security::KeyProvider,
        metadata_only: bool,
    ) -> Result<Self> {
        ensure!(plain_metadata(root)?.is_dir(), "invalid store root");
        ensure!(
            !root.join("index.json.enc").exists(),
            crate::error::Error::new(
                crate::error::ErrorCode::UnsupportedStore,
                "legacy store; use a separate new-format root"
            )
        );
        let root = root.canonicalize()?;
        // Reject unsupported roots before taking a lock or writing recovery data.
        let metadata = claim::persistence::Metadata::open(&root, keys)?;
        for part in [
            "source", "index", "claims", "policies", "store", "bin", ".locks", ".staging",
        ] {
            ensure!(
                plain_metadata(&root.join(part))?.is_dir(),
                "invalid store layout"
            );
        }
        let store = Self {
            policy_fence: None,
            metadata_only,
            session: None,
            root,
            metadata,
            #[cfg(test)]
            before_claim_commit: None,
            #[cfg(test)]
            access_observer: None,
        };
        {
            let _guard = store.lock()?;
        }
        Ok(store)
    }
    pub(crate) fn lock(&self) -> Result<StoreGuard> {
        self.lock_with_timeout(Duration::from_secs(30))
    }
    pub(crate) fn lock_with_timeout(&self, timeout: Duration) -> Result<StoreGuard> {
        if let Some(native) =
            HELD_LOCKS.with(|locks| locks.borrow().get(&self.root).and_then(Weak::upgrade))
        {
            if self.metadata_only {
                self.metadata.require_reconciled()?;
            }
            return Ok(StoreGuard {
                root: self.root.clone(),
                native,
            });
        }
        if self.metadata_only {
            ensure!(
                plain_metadata(&self.root.join(".locks/mutation"))?.is_file(),
                "metadata read requires existing coordination"
            );
        }
        let native = Rc::new(LockGuard::acquire(
            &self.root.join(".locks/mutation"),
            LockMode::Exclusive,
            timeout,
        )?);
        if self.metadata_only {
            self.metadata.require_reconciled()?;
            self.metadata.snapshot()?;
        } else {
            self.recover_locked()?;
        }
        HELD_LOCKS.with(|locks| {
            locks
                .borrow_mut()
                .insert(self.root.clone(), Rc::downgrade(&native));
        });
        Ok(StoreGuard {
            root: self.root.clone(),
            native,
        })
    }
}
