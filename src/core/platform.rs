use super::Store;
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

trait SecretProvider {
    fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>>;
    fn store(&self, secret: &[u8; 32]) -> Result<()>;
}

struct NativeSecret(keyring::Entry);
impl SecretProvider for NativeSecret {
    fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>> {
        match self.0.get_secret() {
            Ok(bytes) => {
                let bytes = Zeroizing::new(bytes);
                Ok(Some(Zeroizing::new(
                    bytes
                        .as_slice()
                        .try_into()
                        .context("invalid native secret length")?,
                )))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error).context("native secret provider failed"),
        }
    }
    fn store(&self, secret: &[u8; 32]) -> Result<()> {
        self.0
            .set_secret(secret)
            .context("native secret provider failed")
    }
}

pub fn user_store_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("user home is unavailable")?
        .join(".saucepan"))
}
pub fn shared_executable() -> Result<PathBuf> {
    Ok(user_store_path()?.join("bin").join(if cfg!(windows) {
        "saucepan.exe"
    } else {
        "saucepan"
    }))
}

impl Store {
    pub fn create_user() -> Result<Self> {
        create(&user_store_path()?, &native()?)
    }
    pub fn open_user() -> Result<Self> {
        open(&user_store_path()?, &native()?)
    }
}

fn native() -> Result<NativeSecret> {
    Ok(NativeSecret(keyring::Entry::new(
        "saucepan.central-source-store.v1",
        "master",
    )?))
}

fn create(root: &Path, provider: &impl SecretProvider) -> Result<Store> {
    let parent = root.parent().context("store parent is missing")?;
    let _lock = crate::utils::lock::FileLock::acquire(
        parent.join(".saucepan-init.lock"),
        std::time::Duration::from_secs(30),
    )?;
    Store::check_new_root(root)?;
    ensure!(
        provider.load()?.is_none(),
        "store secret already exists; refusing replacement"
    );
    let key = crate::utils::crypto::random_bytes()?;
    provider.store(&key)?;
    Store::create_test(root, *key)
}
fn open(root: &Path, provider: &impl SecretProvider) -> Result<Store> {
    let key = provider.load()?.context("store secret is missing")?;
    Store::open_test(root, *key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Memory(RefCell<Option<[u8; 32]>>);
    impl SecretProvider for Memory {
        fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>> {
            Ok(self.0.borrow().map(Zeroizing::new))
        }
        fn store(&self, key: &[u8; 32]) -> Result<()> {
            *self.0.borrow_mut() = Some(*key);
            Ok(())
        }
    }
    struct Unavailable;
    impl SecretProvider for Unavailable {
        fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>> {
            anyhow::bail!("provider unavailable")
        }
        fn store(&self, _: &[u8; 32]) -> Result<()> {
            panic!("must not create replacement key")
        }
    }

    #[test]
    fn explicit_creation_and_opening_never_replace_an_existing_secret() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let provider = Memory(RefCell::new(None));
        create(&root, &provider).unwrap();
        open(&root, &provider).unwrap();
        let key = *provider.0.borrow();
        assert!(create(&root, &provider).is_err());
        std::fs::remove_file(root.join("index.json.enc")).unwrap();
        assert!(open(&root, &provider).is_err());
        assert_eq!(*provider.0.borrow(), key);
    }

    #[test]
    fn provider_failures_and_missing_keys_never_choose_test_or_plaintext_storage() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        assert!(create(&root, &Unavailable).is_err());
        assert!(!root.exists());
        assert!(open(&root, &Unavailable).is_err());
        Store::create_test(&root, [7; 32]).unwrap();
        assert!(open(&root, &Memory(RefCell::new(None))).is_err());
        assert!(open(&root, &Unavailable).is_err());
    }

    #[test]
    fn executable_is_one_user_level_path() {
        let name = if cfg!(windows) {
            "saucepan.exe"
        } else {
            "saucepan"
        };
        assert_eq!(
            shared_executable().unwrap(),
            dirs::home_dir().unwrap().join(".saucepan/bin").join(name)
        );
    }

    #[test]
    fn first_initialization_preserves_an_already_placed_shared_binary() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("bin/saucepan"), b"existing binary").unwrap();
        let provider = Memory(RefCell::new(None));
        create(&root, &provider).unwrap();
        open(&root, &provider).unwrap();
        assert_eq!(
            std::fs::read(root.join("bin/saucepan")).unwrap(),
            b"existing binary"
        );
        assert!(create(&root, &provider).is_err());
    }

    #[test]
    #[ignore = "requires an available native user credential service; writes one temporary test entry"]
    fn native_credential_round_trip() {
        let id = crate::utils::hash::sha256(
            crate::utils::crypto::random_bytes::<32>()
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        let provider = NativeSecret(keyring::Entry::new("saucepan-test", &id).unwrap());
        assert!(provider.load().unwrap().is_none());
        provider.store(&[19; 32]).unwrap();
        let result = provider.load();
        provider.0.delete_credential().unwrap();
        assert_eq!(*result.unwrap().unwrap(), [19; 32]);
    }
}
