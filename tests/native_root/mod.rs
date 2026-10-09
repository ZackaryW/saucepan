//! Explicit native-keyring fixtures; every successfully created test key is retired.
use saucepan::core::{
    Store,
    claim::security::{Authenticator, NativeKeyring},
};
use std::{ops::Deref, path::Path};
pub struct NativeStore {
    store: Store,
    key: uuid::Uuid,
}
impl Deref for NativeStore {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.store
    }
}
impl Drop for NativeStore {
    fn drop(&mut self) {
        if let Err(error) = NativeKeyring.retire(self.key) {
            eprintln!("native fixture key cleanup failed: {error}");
        }
    }
}
#[allow(dead_code)]
pub fn create(root: &Path) -> anyhow::Result<NativeStore> {
    let store = Store::create_with_keys(root, &NativeKeyring)?;
    let key = Authenticator::store_hint(&std::fs::read(root.join("root.json"))?)?;
    Ok(NativeStore { store, key })
}

#[allow(dead_code)]
pub fn open(root: &Path) -> anyhow::Result<NativeStore> {
    let store = Store::open_with_keys(root, &NativeKeyring)?;
    let key = Authenticator::store_hint(&std::fs::read(root.join("root.json"))?)?;
    Ok(NativeStore { store, key })
}
