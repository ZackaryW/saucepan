//! Ordinary native Rust fixtures use basic integrity and forbid custody calls.
use saucepan::{
    core::{
        Store,
        claim::security::{KeyProvider, Secret},
    },
    error::Result,
    models::claim::security::IntegrityMode,
};
use std::path::Path;
pub type NativeStore = Store;
struct NoCustody;
impl KeyProvider for NoCustody {
    fn load(&self, _: uuid::Uuid) -> Result<Option<Secret>> {
        panic!("ordinary fixture accessed native custody")
    }
    fn insert(&self, _: uuid::Uuid, _: &Secret) -> Result<()> {
        panic!("ordinary fixture provisioned native custody")
    }
}
#[allow(dead_code)]
pub fn create(root: &Path) -> anyhow::Result<NativeStore> {
    Store::create_with_integrity_and_keys(root, IntegrityMode::Basic, &NoCustody)
}
#[allow(dead_code)]
pub fn open(root: &Path) -> anyhow::Result<NativeStore> {
    Store::open_with_keys(root, &NoCustody)
}
