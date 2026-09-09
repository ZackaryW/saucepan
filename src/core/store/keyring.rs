//! Native credential custody. There is no file or environment-secret fallback.
use crate::core::models::{Error, ErrorKind, Result};
use zeroize::Zeroizing;

pub(super) trait SecretStore {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>>;
    fn write(&self, secret: &[u8]) -> Result<()>;
    #[cfg(test)]
    fn delete(&self) -> Result<()>;
}

pub(super) struct NativeKeyring {
    entry: keyring::Entry,
}

impl NativeKeyring {
    pub(super) fn new(service: &str, account: &str) -> Result<Self> {
        keyring::Entry::new(service, account)
            .map(|entry| Self { entry })
            .map_err(|_| unavailable())
    }
}

impl SecretStore for NativeKeyring {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        match self.entry.get_secret() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(unavailable()),
        }
    }
    fn write(&self, secret: &[u8]) -> Result<()> {
        self.entry.set_secret(secret).map_err(|_| unavailable())
    }
    #[cfg(test)]
    fn delete(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(unavailable()),
        }
    }
}
fn unavailable() -> Error {
    Error::new(
        ErrorKind::Authority,
        "OS keyring unavailable; unlock Credential Manager/Keychain/Secret Service and retry (no plaintext fallback)",
    )
}

#[cfg(test)]
pub(super) mod testing {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    pub(crate) struct MemorySecrets {
        pub value: RefCell<Option<Zeroizing<Vec<u8>>>>,
        pub unavailable: bool,
    }
    impl SecretStore for MemorySecrets {
        fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
            if self.unavailable {
                return Err(super::unavailable());
            }
            Ok(self
                .value
                .borrow()
                .as_ref()
                .map(|v| Zeroizing::new(v.to_vec())))
        }
        fn write(&self, bytes: &[u8]) -> Result<()> {
            if self.unavailable {
                return Err(super::unavailable());
            }
            *self.value.borrow_mut() = Some(Zeroizing::new(bytes.to_vec()));
            Ok(())
        }
        fn delete(&self) -> Result<()> {
            self.value.borrow_mut().take();
            Ok(())
        }
    }

    /// Explicit native smoke; creates and deletes only a unique test credential.
    #[test]
    #[ignore = "requires an unlocked native OS credential service"]
    fn native_keyring_round_trip() {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).unwrap();
        let account = format!("test-{}", crate::utils::hex(&bytes));
        let store = NativeKeyring::new("org.saucepan.integration-tests", &account).unwrap();
        assert!(store.read().unwrap().is_none());
        struct Cleanup<'a>(&'a NativeKeyring);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete();
            }
        }
        let _cleanup = Cleanup(&store);
        store.write(b"saucepan-ephemeral-smoke-secret").unwrap();
        assert_eq!(
            store.read().unwrap().unwrap().as_slice(),
            b"saucepan-ephemeral-smoke-secret"
        );
        store.delete().unwrap();
        assert!(store.read().unwrap().is_none());
    }
}
