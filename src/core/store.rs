use super::models::*;
use crate::utils::{
    crypto,
    fs::{atomic_write, staged_directory},
    json::sorted_json,
    lock::FileLock,
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use zeroize::Zeroizing;

const MAGIC: &[u8] = b"SAUCE\x01";
const INDEX_CONTEXT: &[u8] = b"saucepan/index/v1";

/// One user store. Each operation locks, authenticates, reads and publishes afresh.
pub struct Store {
    pub(crate) root: PathBuf,
    key: Zeroizing<[u8; 32]>,
    #[cfg(test)]
    pub(crate) fail_publication: std::sync::atomic::AtomicBool,
}

impl Store {
    /// Explicit isolated construction; never selected as a production fallback.
    pub fn create_test(root: impl AsRef<Path>, key: [u8; 32]) -> Result<Self> {
        Self::check_new_root(root.as_ref())?;
        let store = Self {
            root: root.as_ref().to_owned(),
            key: Zeroizing::new(key),
            #[cfg(test)]
            fail_publication: std::sync::atomic::AtomicBool::new(false),
        };
        let index = Index {
            version: FORMAT_VERSION,
            store_id: *crypto::random_bytes()?,
            apps: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            sources: BTreeMap::new(),
        };
        let bytes = store.encode(&index, INDEX_CONTEXT)?;
        let initialize = |stage: &Path| {
            fs::create_dir(stage.join("sources"))?;
            fs::create_dir_all(stage.join("bin"))?;
            atomic_write(stage.join("index.json.enc"), |file| file.write_all(&bytes))
        };
        if store.root.try_exists()? {
            initialize(&store.root)?;
        } else {
            staged_directory(&store.root, initialize)?;
        }
        Ok(store)
    }

    /// Installation may place the executable before the store is initialized.
    pub(crate) fn check_new_root(root: &Path) -> Result<()> {
        if !root.try_exists()? {
            return Ok(());
        }
        ensure!(
            crate::utils::tree::metadata(root)?.is_dir(),
            "invalid store root"
        );
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            ensure!(
                entry.file_name() == "bin" && crate::utils::tree::metadata(&entry.path())?.is_dir(),
                "store already exists or contains unrelated files"
            );
        }
        Ok(())
    }
    pub fn open_test(root: impl AsRef<Path>, key: [u8; 32]) -> Result<Self> {
        let store = Self {
            root: root.as_ref().to_owned(),
            key: Zeroizing::new(key),
            #[cfg(test)]
            fail_publication: std::sync::atomic::AtomicBool::new(false),
        };
        let _lock = store.lock()?;
        store.read()?;
        Ok(store)
    }
    pub fn register(&self, app: &str, settings: AppSettings, filters: Filters) -> Result<AppToken> {
        ensure!(
            !app.trim().is_empty() && !app.chars().any(char::is_control),
            "invalid app name"
        );
        validate_filters(&filters)?;
        let _lock = self.lock()?;
        let mut index = self.read()?;
        ensure!(!index.apps.contains_key(app), "app is already registered");
        let proof = AppToken {
            version: FORMAT_VERSION,
            app: app.to_owned(),
            token: *crypto::random_bytes()?,
        };
        let token_binding = crypto::authenticate(
            self.key_for(&index, b"caller-token")?.as_ref(),
            sorted_json(&proof)?.as_slice(),
        )?;
        index.apps.insert(
            app.into(),
            RegisteredApp {
                settings,
                filters,
                touched: BTreeSet::new(),
                token_binding,
                data_hmac: [0; 32],
            },
        );
        self.save(&mut index)?;
        Ok(proof)
    }
    pub fn configure(
        &self,
        context: &AppContext,
        settings: AppSettings,
        filters: Filters,
    ) -> Result<()> {
        validate_filters(&filters)?;
        let _lock = self.lock()?;
        let mut index = self.read()?;
        self.access(&index, context)?;
        let app = index
            .apps
            .get_mut(&context.app)
            .context("app is not registered")?;
        app.settings = settings;
        app.filters = filters;
        self.save(&mut index)
    }
    pub fn view(&self, context: &AppContext) -> Result<AppView> {
        let _lock = self.lock()?;
        let index = self.read()?;
        self.access(&index, context)?;
        Self::select(&index, &context.app)
    }
    pub fn verify_view(&self, context: &AppContext, view: &AppView) -> Result<()> {
        ensure!(
            context.proof.is_some(),
            "verification requires a caller proof"
        );
        let _lock = self.lock()?;
        let index = self.read()?;
        let app = self.access(&index, context)?;
        ensure!(
            context.app == view.app && view.version == FORMAT_VERSION,
            "view identity/version mismatch"
        );
        crypto::verify(
            self.key_for(&index, b"app-view")?.as_ref(),
            sorted_json(&(view, &app.touched))?.as_slice(),
            &app.data_hmac,
        )?;
        Ok(())
    }
    pub fn write_marker(&self, proof: &AppToken, path: impl AsRef<Path>) -> Result<()> {
        self.view(&AppContext::authenticated(proof.clone()))?;
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut stage = tempfile::NamedTempFile::new_in(parent)?;
        stage.write_all(&sorted_json(proof)?)?;
        stage.as_file().sync_all()?;
        stage.persist_noclobber(path)?;
        Ok(())
    }
    pub fn read_marker(path: impl AsRef<Path>) -> Result<AppToken> {
        let mut bytes = Vec::new();
        fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 65536, "marker too large");
        let proof: AppToken = serde_json::from_slice(&bytes)?;
        ensure!(proof.version == FORMAT_VERSION, "unsupported proof version");
        Ok(proof)
    }

    pub(crate) fn lock(&self) -> Result<FileLock> {
        ensure!(
            self.root.join("index.json.enc").is_file(),
            "store index is missing"
        );
        Ok(FileLock::acquire(
            self.root.join("index.lock"),
            Duration::from_secs(30),
        )?)
    }
    pub(crate) fn read(&self) -> Result<Index> {
        let index: Index =
            self.decode(&fs::read(self.root.join("index.json.enc"))?, INDEX_CONTEXT)?;
        ensure!(index.version == FORMAT_VERSION, "unsupported index version");
        for (name, app) in &index.apps {
            crypto::verify(
                self.key_for(&index, b"app-view")?.as_ref(),
                sorted_json(&(Self::select(&index, name)?, &app.touched))?.as_slice(),
                &app.data_hmac,
            )?;
        }
        for (id, state) in &index.sources {
            let context = format!("saucepan/source/v1/{id}");
            let recorded: SourceState = self.decode(
                &fs::read(self.source_index_path(id, state)?)?,
                context.as_bytes(),
            )?;
            ensure!(
                &recorded == state,
                "source index does not match committed central state"
            );
        }
        Ok(index)
    }
    pub(crate) fn save(&self, index: &mut Index) -> Result<()> {
        let key = self.key_for(index, b"app-view")?;
        for name in index.apps.keys().cloned().collect::<Vec<_>>() {
            let tag = crypto::authenticate(
                key.as_ref(),
                sorted_json(&(Self::select(index, &name)?, &index.apps[&name].touched))?.as_slice(),
            )?;
            index
                .apps
                .get_mut(&name)
                .context("app disappeared")?
                .data_hmac = tag;
        }
        for (id, state) in &index.sources {
            let path = self.source_index_path(id, state)?;
            fs::create_dir_all(path.parent().context("missing source index parent")?)?;
            if !path.try_exists()? {
                let context = format!("saucepan/source/v1/{id}");
                let bytes = self.encode(state, context.as_bytes())?;
                atomic_write(path, |file| file.write_all(&bytes))?;
            } else {
                let context = format!("saucepan/source/v1/{id}");
                let existing: SourceState = self.decode(&fs::read(path)?, context.as_bytes())?;
                ensure!(
                    &existing == state,
                    "prepared source index does not match incoming state"
                );
            }
        }
        let bytes = self.encode(index, INDEX_CONTEXT)?;
        #[cfg(test)]
        ensure!(
            !self
                .fail_publication
                .load(std::sync::atomic::Ordering::Relaxed),
            "injected publication failure"
        );
        atomic_write(self.root.join("index.json.enc"), |file| {
            file.write_all(&bytes)
        })?;
        Ok(())
    }
    fn source_index_path(&self, id: &str, state: &SourceState) -> Result<PathBuf> {
        Ok(self
            .root
            .join("sources")
            .join(id)
            .join("indexes")
            .join(format!("{}.json.enc", super::content::digest(state)?)))
    }
    pub(crate) fn access<'a>(
        &self,
        index: &'a Index,
        context: &AppContext,
    ) -> Result<&'a RegisteredApp> {
        ensure!(
            context.version == FORMAT_VERSION && context.restrictions.is_empty(),
            "unsupported request version or restrictions"
        );
        let app = index
            .apps
            .get(&context.app)
            .context("app is not registered")?;
        ensure!(
            !context.authoritative || context.proof.is_some(),
            "authoritative request requires a caller proof"
        );
        if let Some(proof) = &context.proof {
            ensure!(
                proof.version == FORMAT_VERSION && proof.app == context.app,
                "proof identity/version mismatch"
            );
            crypto::verify(
                self.key_for(index, b"caller-token")?.as_ref(),
                sorted_json(proof)?.as_slice(),
                &app.token_binding,
            )?;
        }
        let mut checked = BTreeSet::new();
        for artifact in Self::select(index, &context.app)?.entries.values() {
            if matches!(artifact.source, Source::Git { .. }) && checked.insert(&artifact.source_id)
            {
                let source = index
                    .sources
                    .get(&artifact.source_id)
                    .context("artifact source is missing")?;
                super::sources::git::verify_origin(
                    &source.source,
                    &self
                        .root
                        .join("sources")
                        .join(&artifact.source_id)
                        .join("repo"),
                )?;
            }
        }
        Ok(app)
    }
    pub(crate) fn select(index: &Index, name: &str) -> Result<AppView> {
        let app = index.apps.get(name).context("app is not registered")?;
        let entries = app
            .touched
            .iter()
            .filter_map(|id| {
                index
                    .artifacts
                    .get(id)
                    .filter(|artifact| {
                        super::policies::selected(
                            &app.filters,
                            &artifact.source_id,
                            &artifact.source,
                        )
                    })
                    .map(|artifact| (id.clone(), artifact.clone()))
            })
            .collect();
        Ok(AppView {
            version: FORMAT_VERSION,
            app: name.into(),
            settings: app.settings.clone(),
            filters: app.filters.clone(),
            entries,
        })
    }
    fn key_for(&self, index: &Index, purpose: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        Ok(crypto::derive_key(
            self.key.as_ref(),
            Some(&index.store_id),
            purpose,
        )?)
    }
    pub(crate) fn encode(&self, value: &impl serde::Serialize, context: &[u8]) -> Result<Vec<u8>> {
        let key = crypto::derive_key(self.key.as_ref(), None, b"saucepan/encryption/v1")?;
        let plaintext = Zeroizing::new(sorted_json(value)?);
        let sealed = crypto::seal(&key, &plaintext, context)?;
        Ok([MAGIC, &sealed.nonce, &sealed.ciphertext].concat())
    }
    pub(crate) fn decode<T: serde::de::DeserializeOwned>(
        &self,
        bytes: &[u8],
        context: &[u8],
    ) -> Result<T> {
        ensure!(
            bytes.len() >= MAGIC.len() + 24 + 16 && bytes.starts_with(MAGIC),
            "unsupported or truncated encrypted format"
        );
        let key = crypto::derive_key(self.key.as_ref(), None, b"saucepan/encryption/v1")?;
        let sealed = crypto::Sealed {
            nonce: bytes[6..30].try_into()?,
            ciphertext: bytes[30..].to_vec(),
        };
        Ok(serde_json::from_slice(&crypto::open(
            &key, &sealed, context,
        )?)?)
    }
}

fn validate_filters(filters: &Filters) -> Result<()> {
    ensure!(
        filters
            .providers
            .iter()
            .all(|p| matches!(p.as_str(), "git" | "url" | "local")),
        "unsupported filter provider"
    );
    ensure!(
        filters.source_ids.iter().all(|id| id.len() == 64
            && id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())),
        "filter source IDs must be lowercase SHA-256"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn encrypted_round_trip_uses_central_settings_and_stable_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let store = Store::create_test(&root, [7; 32]).unwrap();
        let token = store
            .register("app-a", AppSettings::default(), Filters::default())
            .unwrap();
        let context = AppContext::authenticated(token.clone());
        let before = store.view(&context).unwrap();
        fs::write(
            root.join("saucepan.toml"),
            "invalid TOML cannot affect settings",
        )
        .unwrap();
        let settings = AppSettings {
            retain_snapshots: false,
            verify_content: true,
            allow_local_fallback: true,
        };
        store
            .configure(&context, settings.clone(), Filters::default())
            .unwrap();
        assert!(store.verify_view(&context, &before).is_err());
        let reopened = Store::open_test(&root, [7; 32]).unwrap();
        let view = reopened.view(&context).unwrap();
        assert_eq!(view.settings, settings);
        reopened.verify_view(&context, &view).unwrap();
        let bytes = fs::read(root.join("index.json.enc")).unwrap();
        assert!(!bytes.windows(5).any(|bytes| bytes == b"app-a"));
        assert!(Store::create_test(&root, [7; 32]).is_err());
        assert!(Store::open_test(dir.path().join("missing"), [7; 32]).is_err());
    }

    #[test]
    fn tamper_wrong_key_and_missing_index_fail_without_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        Store::create_test(&root, [7; 32]).unwrap();
        let path = root.join("index.json.enc");
        let original = fs::read(&path).unwrap();
        assert!(Store::open_test(&root, [8; 32]).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        let mut bad = original;
        *bad.last_mut().unwrap() ^= 1;
        fs::write(&path, &bad).unwrap();
        assert!(Store::open_test(&root, [7; 32]).is_err());
        assert_eq!(fs::read(&path).unwrap(), bad);
        fs::remove_file(&path).unwrap();
        assert!(Store::open_test(&root, [7; 32]).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn app_and_store_token_binding_rejects_substitution_and_unknown_formats() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create_test(dir.path().join("one"), [7; 32]).unwrap();
        let a = store
            .register("a", AppSettings::default(), Filters::default())
            .unwrap();
        let b = store
            .register("b", AppSettings::default(), Filters::default())
            .unwrap();
        let mut context = AppContext::authenticated(a.clone());
        context.app = "b".into();
        assert!(store.view(&context).is_err());
        context = AppContext::authenticated(b);
        context.version += 1;
        assert!(store.view(&context).is_err());
        context = AppContext::authenticated(a.clone());
        context.restrictions.push("read-only".into());
        assert!(store.view(&context).is_err());
        context = AppContext {
            authoritative: true,
            ..AppContext::ordinary("a")
        };
        assert!(store.view(&context).is_err());
        let other = Store::create_test(dir.path().join("two"), [7; 32]).unwrap();
        other
            .register("a", AppSettings::default(), Filters::default())
            .unwrap();
        assert!(other.view(&AppContext::authenticated(a.clone())).is_err());
        let marker = dir.path().join(".saucepanhash");
        store.write_marker(&a, &marker).unwrap();
        let read = Store::read_marker(&marker).unwrap();
        assert!(read == a);
        assert!(store.write_marker(&a, &marker).is_err());
        let mut view = store.view(&AppContext::authenticated(a.clone())).unwrap();
        view.filters.providers.insert("git".into());
        assert!(
            store
                .verify_view(&AppContext::authenticated(a), &view)
                .is_err()
        );
        assert_eq!(
            store.view(&AppContext::ordinary("b")).unwrap().settings,
            AppSettings::default()
        );
    }

    #[test]
    fn unsupported_persisted_versions_and_internal_scope_tampering_fail() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let store = Store::create_test(&root, [7; 32]).unwrap();
        store
            .register("a", AppSettings::default(), Filters::default())
            .unwrap();
        let mut index = store.read().unwrap();
        index
            .apps
            .get_mut("a")
            .unwrap()
            .touched
            .insert("unselected-record".into());
        fs::write(
            root.join("index.json.enc"),
            store.encode(&index, INDEX_CONTEXT).unwrap(),
        )
        .unwrap();
        assert!(store.read().is_err());
        index.version += 1;
        fs::write(
            root.join("index.json.enc"),
            store.encode(&index, INDEX_CONTEXT).unwrap(),
        )
        .unwrap();
        assert!(store.read().is_err());
    }
}
