//! Coordinated authenticated metadata. Every method is called while holding the
//! store mutation lock; filesystem content publication remains the store's job.
use super::security::{
    Authenticator, Identity, KeyProvider, Kind, MAX_RECORD_BYTES, integrity_error,
};
use crate::models::claim::security::IntegrityMode;
use crate::{
    error::{Error, ErrorCode, Result},
    utils::{
        fs::atomic_write,
        path::{beneath, plain_metadata},
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

const CONTRACT: &str = "saucepan.flat-policy-resources";
const ROOT: &str = "root.json";
const PENDING: &str = ".staging/metadata-transaction.json";
const INITIALIZING: &str = ".staging/initializing.json";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    contract: String,
    store: Uuid,
    mode: IntegrityMode,
}
fn reservation(bytes: &[u8]) -> Result<Reservation> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
    if value.get("contract").and_then(serde_json::Value::as_str) != Some(CONTRACT) {
        return Err(Error::new(
            ErrorCode::UnsupportedStore,
            "initialization belongs to an earlier store contract; use a fresh root",
        ));
    }
    serde_json::from_value(value).map_err(|_| integrity_error())
}
fn require_mode(actual: IntegrityMode, requested: IntegrityMode) -> Result<()> {
    if actual != requested {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "store integrity mode cannot be changed",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub identity: Identity,
    pub generation: u64,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub contract: String,
    pub generation: u64,
    pub records: BTreeMap<String, Reference>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    parent: String,
    next: Box<RawValue>,
    writes: BTreeMap<String, Box<RawValue>>,
    deletes: BTreeSet<String>,
    content: Option<ContentPublication>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentPublication {
    pub stage: String,
    pub output: Uuid,
    /// Absent for acquired outputs; present for source-owned retrieval material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_source: Option<Uuid>,
    pub files: BTreeMap<String, crate::models::store::FileEntry>,
}

/// A snapshot is trusted only after both the catalog and every referenced record
/// have been verified. Its in-memory payloads are immutable observations.
pub struct Snapshot {
    pub catalog: Catalog,
    pub digest: String,
    records: BTreeMap<String, Vec<u8>>,
}
impl Snapshot {
    pub fn contains(&self, identity: Identity) -> Result<bool> {
        Ok(self
            .catalog
            .records
            .get(&record_path(identity)?)
            .is_some_and(|r| r.identity == identity))
    }
    pub fn read<T: DeserializeOwned>(&self, auth: &Authenticator, identity: Identity) -> Result<T> {
        let path = record_path(identity)?;
        let bytes = self
            .records
            .get(&path)
            .ok_or_else(|| missing(identity.kind))?;
        auth.verify(identity, bytes).map(|(_, value)| value)
    }
    pub fn digest(&self, identity: Identity) -> Result<&str> {
        let path = record_path(identity)?;
        self.catalog
            .records
            .get(&path)
            .map(|r| r.digest.as_str())
            .ok_or_else(|| missing(identity.kind))
    }
}

/// An explicitly constructed persistence boundary. Store APIs provide locking and
/// operation authorization; possession of this low-level object is administration.
#[derive(Clone)]
pub struct Metadata {
    root: PathBuf,
    auth: Arc<Authenticator>,
}
impl Metadata {
    pub(crate) fn check_initialization_contract(root: &Path) -> Result<()> {
        let marker = root.join(INITIALIZING);
        if marker.exists() {
            reservation(&bounded_read(&marker)?)?;
        }
        Ok(())
    }
    pub(crate) fn require_reconciled(&self) -> Result<()> {
        if beneath(&self.root, PENDING)?.try_exists()?
            || beneath(&self.root, INITIALIZING)?.try_exists()?
        {
            return Err(Error::new(
                ErrorCode::RecoveryRequired,
                "reconcile pending metadata separately before fresh retrieval",
            ));
        }
        Ok(())
    }
    pub fn authenticator(&self) -> &Authenticator {
        &self.auth
    }

    /// Initialize an otherwise empty prepared root. A reserved UUID lets a retry
    /// reuse a key provisioned before a crash. Existing live roots are never reset.
    pub fn initialize(root: &Path, keys: &dyn KeyProvider) -> Result<Self> {
        Self::initialize_with_mode(root, IntegrityMode::Keyring, keys)
    }
    pub fn initialize_with_mode(
        root: &Path,
        mode: IntegrityMode,
        keys: &dyn KeyProvider,
    ) -> Result<Self> {
        if root.join(ROOT).exists() {
            let metadata = Self::open(root, keys)?;
            require_mode(metadata.auth.mode(), mode)?;
            let marker = beneath(root, INITIALIZING)?;
            if marker.exists() {
                let reserved = reservation(&bounded_read(&marker)?)?;
                require_mode(reserved.mode, mode)?;
                if reserved.store != metadata.auth.store() {
                    return Err(integrity_error());
                }
            }
            metadata.recover()?;
            metadata.snapshot()?;
            if marker.exists() {
                fs::remove_file(marker)?;
            }
            return Ok(metadata);
        }
        for part in ["source", "index", "claims", "policies", "store", "bin"] {
            let path = root.join(part);
            if path.exists() && fs::read_dir(path)?.next().is_some() {
                return Err(Error::new(
                    ErrorCode::UnsupportedStore,
                    "initialization requires an empty store",
                ));
            }
        }
        let marker = beneath(root, INITIALIZING)?;
        let store = if marker.exists() {
            let reserved = reservation(&bounded_read(&marker)?)?;
            require_mode(reserved.mode, mode)?;
            reserved.store
        } else {
            let store = Uuid::new_v4();
            atomic_write(&marker, |file| {
                serde_json::to_writer(
                    file,
                    &Reservation {
                        contract: CONTRACT.into(),
                        store,
                        mode,
                    },
                )
                .map_err(std::io::Error::other)
            })?;
            store
        };
        let auth = Arc::new(match mode {
            IntegrityMode::Basic => Authenticator::basic(store),
            IntegrityMode::Keyring => Authenticator::initialize(store, keys)?,
        });
        let metadata = Self {
            root: root.to_path_buf(),
            auth,
        };
        let catalog = Catalog {
            contract: CONTRACT.into(),
            generation: 0,
            records: BTreeMap::new(),
        };
        let bytes = metadata.auth.sign(Identity::catalog(), 0, &catalog)?;
        metadata.write(ROOT, &bytes)?;
        fs::remove_file(marker)?;
        Ok(metadata)
    }

    /// No key creation and no unsigned fallback occur here.
    pub fn open(root: &Path, keys: &dyn KeyProvider) -> Result<Self> {
        if !root.join(ROOT).exists() {
            return Err(Error::new(
                ErrorCode::UnsupportedStore,
                "store has no authenticated catalog; use a fresh root",
            ));
        }
        let bytes = bounded_read(&beneath(root, ROOT)?)?;
        // This untrusted hint is used only to reject unsupported contracts. All
        // accepted bytes still undergo normal verification after custody opens.
        #[derive(Deserialize)]
        struct Hint {
            payload: ContractHint,
        }
        #[derive(Deserialize)]
        struct ContractHint {
            contract: String,
        }
        let hint: Hint = serde_json::from_slice(&bytes).map_err(|_| integrity_error())?;
        if hint.payload.contract != CONTRACT {
            return Err(Error::new(
                ErrorCode::UnsupportedStore,
                "earlier nested-policy stores are unsupported; use a fresh root",
            ));
        }
        let (store, mode) = Authenticator::root_hint(&bytes)?;
        let auth = Arc::new(match mode {
            IntegrityMode::Basic => Authenticator::basic(store),
            IntegrityMode::Keyring => Authenticator::load(store, keys)?,
        });
        let metadata = Self {
            root: root.to_path_buf(),
            auth,
        };
        metadata.catalog(&bytes)?;
        Ok(metadata)
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        let bytes = self.read(ROOT)?;
        let catalog = self.catalog(&bytes)?;
        let mut records = BTreeMap::new();
        for (path, reference) in &catalog.records {
            let bytes = self.read_expected(path)?;
            self.verify_reference(path, reference, &bytes)?;
            records.insert(path.clone(), bytes);
        }
        Ok(Snapshot {
            catalog,
            digest: digest(&bytes),
            records,
        })
    }

    /// Build the entire next generation before writing the journal. All proposed
    /// record payloads remain readable JSON inside its signed envelope.
    pub fn publish(
        &self,
        snapshot: &Snapshot,
        writes: Vec<(Identity, serde_json::Value)>,
        deletes: Vec<Identity>,
    ) -> Result<()> {
        self.publish_observed(snapshot, writes, deletes, |_| Ok(()))
    }

    /// Observer is a fault-injection boundary; an error simulates process death.
    /// Recovery must run before another snapshot is consumed after a failed publish.
    pub fn publish_observed(
        &self,
        snapshot: &Snapshot,
        writes: Vec<(Identity, serde_json::Value)>,
        deletes: Vec<Identity>,
        observer: impl FnMut(Publication) -> Result<()>,
    ) -> Result<()> {
        self.publish_staged(snapshot, writes, deletes, None, observer)
    }

    pub fn publish_staged(
        &self,
        snapshot: &Snapshot,
        writes: Vec<(Identity, serde_json::Value)>,
        deletes: Vec<Identity>,
        content: Option<ContentPublication>,
        mut observer: impl FnMut(Publication) -> Result<()>,
    ) -> Result<()> {
        if self.root.join(PENDING).exists() {
            return Err(integrity_error());
        }
        if digest(&self.read(ROOT)?) != snapshot.digest {
            return Err(Error::new(
                ErrorCode::RevisionConflict,
                "metadata generation changed",
            ));
        }
        // Recheck persisted membership before authoring a transition: an external
        // edit must not be blessed just because an earlier snapshot was valid.
        self.snapshot()?;
        let generation = snapshot
            .catalog
            .generation
            .checked_add(1)
            .ok_or_else(integrity_error)?;
        let mut next = snapshot.catalog.clone();
        next.generation = generation;
        let mut transaction = Transaction {
            parent: snapshot.digest.clone(),
            next: RawValue::from_string("null".into()).map_err(|_| integrity_error())?,
            writes: BTreeMap::new(),
            deletes: BTreeSet::new(),
            content,
        };
        for identity in deletes {
            if identity.kind == Kind::Policy {
                return Err(Error::new(
                    ErrorCode::PolicyConflict,
                    "policy definitions cannot be deleted",
                ));
            }
            let path = record_path(identity)?;
            let existing = next
                .records
                .remove(&path)
                .ok_or_else(|| missing(identity.kind))?;
            if existing.identity != identity || !transaction.deletes.insert(path) {
                return Err(integrity_error());
            }
        }
        for (identity, value) in writes {
            if identity.kind == Kind::Policy && snapshot.contains(identity)? {
                let previous: serde_json::Value = snapshot.read(&self.auth, identity)?;
                if previous != value {
                    return Err(Error::new(
                        ErrorCode::PolicyConflict,
                        "policy definitions are immutable",
                    ));
                }
            }
            let path = record_path(identity)?;
            if transaction.deletes.contains(&path) || transaction.writes.contains_key(&path) {
                return Err(integrity_error());
            }
            let bytes = self.auth.sign(identity, generation, &value)?;
            next.records.insert(
                path.clone(),
                Reference {
                    identity,
                    generation,
                    digest: digest(&bytes),
                },
            );
            transaction.writes.insert(path, raw(bytes)?);
        }
        transaction.next = raw(self.auth.sign(Identity::catalog(), generation, &next)?)?;
        let bytes = self
            .auth
            .sign(transaction_identity(), generation, &transaction)?;
        self.write(PENDING, &bytes)?;
        observer(Publication::Journal)?;
        self.recover_observed(observer)
    }

    pub fn recover(&self) -> Result<()> {
        self.recover_observed(|_| Ok(()))
    }
    pub fn recover_observed(
        &self,
        mut observer: impl FnMut(Publication) -> Result<()>,
    ) -> Result<()> {
        let pending = beneath(&self.root, PENDING)?;
        if !pending.exists() {
            return Ok(());
        }
        let bytes = bounded_read(&pending)?;
        let (generation, transaction): (u64, Transaction) =
            self.auth.verify(transaction_identity(), &bytes)?;
        let next_bytes = transaction.next.get().as_bytes();
        let next = self.catalog(next_bytes)?;
        if generation != next.generation {
            return Err(integrity_error());
        }
        let current_bytes = self.read(ROOT)?;
        let current = self.catalog(&current_bytes)?;
        let current_digest = digest(&current_bytes);
        let already_published = current_digest == digest(next_bytes);
        if !already_published
            && (current_digest != transaction.parent
                || current.generation.checked_add(1) != Some(generation))
        {
            return Err(integrity_error());
        }
        // Authenticate and validate the complete intended state before any effect.
        // During replay old files may already have been replaced/deleted.
        for (path, reference) in &next.records {
            let bytes = match transaction.writes.get(path) {
                Some(value) => value.get().as_bytes().to_vec(),
                None => self.read_expected(path)?,
            };
            self.verify_reference(path, reference, &bytes)?;
        }
        for path in transaction.writes.keys() {
            if !next.records.contains_key(path) || transaction.deletes.contains(path) {
                return Err(integrity_error());
            }
        }
        for path in &transaction.deletes {
            if next.records.contains_key(path) {
                return Err(integrity_error());
            }
            if !already_published && !current.records.contains_key(path) {
                return Err(integrity_error());
            }
            crate::utils::path::relative_path(path).map_err(|_| integrity_error())?;
        }
        // Omitted mutations may not disappear or change a reference.
        if !already_published {
            for (path, reference) in &current.records {
                if !transaction.writes.contains_key(path)
                    && !transaction.deletes.contains(path)
                    && next.records.get(path).is_none_or(|next| {
                        next.digest != reference.digest
                            || next.identity != reference.identity
                            || next.generation != reference.generation
                    })
                {
                    return Err(integrity_error());
                }
            }
        }
        if let Some(content) = &transaction.content {
            if content.stage.is_empty() || content.stage.contains(['/', '\\']) {
                return Err(integrity_error());
            }
            let input = beneath(&self.root, &format!(".staging/{}/content", content.stage))?;
            let relative = match content.retrieval_source {
                Some(source) => format!("source/{source}/retrieval/{}", content.output),
                None => format!("store/{}", content.output),
            };
            let output = beneath(&self.root, &relative)?;
            let path = if output.exists() { &output } else { &input };
            if !path.exists() {
                // No content or metadata was published. An abandoned prepared
                // directory can be discarded only while the entire old state is intact.
                if already_published {
                    return Err(integrity_error());
                }
                self.snapshot()?;
                fs::remove_file(pending)?;
                return Ok(());
            }
            if crate::core::store::manifest(path).map_err(|_| integrity_error())? != content.files {
                return Err(integrity_error());
            }
            if !output.exists() {
                fs::create_dir_all(output.parent().ok_or_else(integrity_error)?)?;
                fs::rename(input, output)?;
            }
            observer(Publication::Content)?;
        }
        for (path, value) in &transaction.writes {
            self.write(path, value.get().as_bytes())?;
            observer(Publication::Record(path.clone()))?;
        }
        for path in &transaction.deletes {
            match fs::remove_file(beneath(&self.root, path)?) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
            observer(Publication::Deletion(path.clone()))?;
        }
        self.write(ROOT, next_bytes)?;
        observer(Publication::Catalog)?;
        self.snapshot()?;
        fs::remove_file(pending)?;
        observer(Publication::Complete)?;
        Ok(())
    }

    fn catalog(&self, bytes: &[u8]) -> Result<Catalog> {
        let (generation, catalog): (u64, Catalog) = self.auth.verify(Identity::catalog(), bytes)?;
        if catalog.contract != CONTRACT || generation != catalog.generation {
            return Err(integrity_error());
        }
        for (path, reference) in &catalog.records {
            if record_path(reference.identity)? != *path
                || reference.generation > generation
                || reference.digest.len() != 64
                || !reference.digest.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(integrity_error());
            }
        }
        Ok(catalog)
    }
    fn verify_reference(&self, path: &str, reference: &Reference, bytes: &[u8]) -> Result<()> {
        if record_path(reference.identity)? != path || digest(bytes) != reference.digest {
            return Err(integrity_error());
        }
        let (generation, _): (u64, Box<RawValue>) = self.auth.verify(reference.identity, bytes)?;
        if generation != reference.generation {
            return Err(integrity_error());
        }
        Ok(())
    }
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        bounded_read(&beneath(&self.root, path)?)
    }
    fn read_expected(&self, path: &str) -> Result<Vec<u8>> {
        self.read(path).map_err(|e| {
            if e.code == ErrorCode::IoError {
                integrity_error()
            } else {
                e
            }
        })
    }
    fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let path = beneath(&self.root, path)?;
        fs::create_dir_all(path.parent().ok_or_else(integrity_error)?)?;
        atomic_write(&path, |file| file.write_all(bytes))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Publication {
    Journal,
    Content,
    Record(String),
    Deletion(String),
    Catalog,
    Complete,
}

pub fn record_path(identity: Identity) -> Result<String> {
    match (identity.kind, identity.owner, identity.member) {
        (Kind::Application, Some(app), None) => Ok(format!("index/{app}/index.json")),
        (Kind::Output | Kind::Retrieval | Kind::Acquisition, None, Some(id)) => {
            Ok(format!("index/{id}/index.json"))
        }
        (Kind::Source, None, Some(id)) => Ok(format!("source/{id}.json")),
        (Kind::Policy, None, Some(id)) => Ok(format!("policies/{id}.json")),
        (Kind::Registry, Some(app), None) => Ok(format!("claims/{app}/index.json")),
        (Kind::Claim, Some(app), Some(id)) => Ok(format!("claims/{app}/{id}.json")),
        (Kind::Operation, _, Some(id)) => Ok(format!(".staging/lifecycle/{id}/index.json")),
        _ => Err(integrity_error()),
    }
}
pub fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn transaction_identity() -> Identity {
    Identity {
        kind: Kind::Transaction,
        owner: None,
        member: None,
    }
}
fn raw(bytes: Vec<u8>) -> Result<Box<RawValue>> {
    RawValue::from_string(String::from_utf8(bytes).map_err(|_| integrity_error())?)
        .map_err(|_| integrity_error())
}
pub fn bounded_read(path: &Path) -> Result<Vec<u8>> {
    if !plain_metadata(path)?.is_file() {
        return Err(integrity_error());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(integrity_error());
    }
    Ok(bytes)
}
fn missing(kind: Kind) -> Error {
    let code = match kind {
        Kind::Application => ErrorCode::UnknownApplication,
        Kind::Claim => ErrorCode::UnknownClaim,
        Kind::Output => ErrorCode::UnknownOutput,
        Kind::Source => ErrorCode::SourceError,
        _ => ErrorCode::IntegrityError,
    };
    Error::new(code, "metadata record is not registered")
}
