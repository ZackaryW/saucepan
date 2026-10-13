//! Authenticated readable records and native secret custody. No encryption or
//! policy decisions live here; callers supply the expected record identity.
use crate::models::claim::security::IntegrityMode;
use crate::{
    error::{Error, ErrorCode, Result},
    models::claim::security::{Credential, CredentialPolicy},
    utils::crypto,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::{fmt, sync::Mutex};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Maximum encoded metadata record size. Content files are not subject to this limit.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const SERVICE: &str = "saucepan.metadata-integrity";

/// Protected secret storage. Basic stores never access this provider.
/// Implementations must not overwrite an existing key in `insert`.
pub trait KeyProvider: Send + Sync {
    fn load(&self, store: Uuid) -> Result<Option<Secret>>;
    fn insert(&self, store: Uuid, secret: &Secret) -> Result<()>;
}

pub struct Secret(Zeroizing<[u8; 32]>);
impl Secret {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| key_error())?;
        Ok(Self(Zeroizing::new(bytes)))
    }
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn generate() -> Result<Self> {
        crypto::random_bytes().map(Self).map_err(|_| key_error())
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

#[derive(Debug, Default)]
pub struct NativeKeyring;
static NATIVE_KEYS: Mutex<()> = Mutex::new(());
impl NativeKeyring {
    /// Trusted custody administration, used to retire explicitly owned fixture
    /// keys. Removing a live store's key makes its metadata unreadable.
    pub fn retire(&self, store: Uuid) -> Result<()> {
        let _guard = native_coordination()?;
        keyring::Entry::new(SERVICE, &store.to_string())
            .map_err(|error| native_error("entry", error))?
            .delete_credential()
            .map_err(|error| native_error("retire", error))
    }
}
fn native_coordination() -> Result<(
    std::sync::MutexGuard<'static, ()>,
    crate::utils::lock::LockGuard,
)> {
    let local = NATIVE_KEYS
        .lock()
        .map_err(|_| custody_error("coordinate", "poisoned_lock"))?;
    #[cfg(unix)]
    // SAFETY: geteuid has no arguments or memory safety preconditions.
    let name = format!("saucepan-keyring-{}.lock", unsafe { libc::geteuid() });
    #[cfg(not(unix))]
    let name = "saucepan-keyring.lock";
    let path = std::env::temp_dir().join(name);
    if path.exists() {
        crate::utils::path::plain_metadata(&path)
            .map_err(|_| custody_error("coordinate", "invalid_lock_path"))?;
    }
    let external = crate::utils::lock::LockGuard::acquire(
        &path,
        crate::utils::lock::LockMode::Exclusive,
        std::time::Duration::from_secs(30),
    )
    .map_err(|_| custody_error("coordinate", "lock_unavailable"))?;
    Ok((local, external))
}
impl KeyProvider for NativeKeyring {
    fn load(&self, store: Uuid) -> Result<Option<Secret>> {
        let _guard = native_coordination()?;
        let entry = keyring::Entry::new(SERVICE, &store.to_string())
            .map_err(|error| native_error("entry", error))?;
        match entry.get_secret() {
            Ok(bytes) => Secret::from_bytes(&Zeroizing::new(bytes))
                .map(Some)
                .map_err(|_| custody_error("load", "invalid_key_length")),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(native_error("load", error)),
        }
    }
    fn insert(&self, store: Uuid, secret: &Secret) -> Result<()> {
        let _guard = native_coordination()?;
        // Initialization holds the store's mutation lock. The randomly allocated
        // store UUID is reserved before provisioning and reused after a crash.
        let entry = keyring::Entry::new(SERVICE, &store.to_string())
            .map_err(|error| native_error("entry", error))?;
        match entry.get_secret() {
            Err(keyring::Error::NoEntry) => {
                entry
                    .set_secret(secret.expose())
                    .map_err(|error| native_error("insert", error))?;
                let persisted = Zeroizing::new(
                    entry
                        .get_secret()
                        .map_err(|error| native_error("read_back", error))?,
                );
                if persisted.as_slice() != secret.expose() {
                    return Err(custody_error("read_back", "key_mismatch"));
                }
                Ok(())
            }
            Ok(bytes) => {
                let _discard = Zeroizing::new(bytes);
                Err(custody_error("insert", "already_exists"))
            }
            Err(error) => Err(native_error("check_existing", error)),
        }
    }
}

fn custody_error(operation: &str, reason: &str) -> Error {
    let mut error = Error::new(
        ErrorCode::KeyUnavailable,
        format!("metadata signing key is unavailable ({operation}: {reason})"),
    );
    error.details.insert("operation".into(), operation.into());
    error.details.insert("reason".into(), reason.into());
    error
}
fn native_error(operation: &str, error: keyring::Error) -> Error {
    // Backend error payloads may include secret bytes or platform descriptions.
    // Report only controlled categories, never Display/Debug of the backend.
    let reason = match error {
        keyring::Error::NoEntry => "missing_key",
        keyring::Error::NoStorageAccess(_) => "storage_unavailable",
        keyring::Error::PlatformFailure(_) => "platform_failure",
        keyring::Error::BadEncoding(_) | keyring::Error::BadDataFormat(_, _) => {
            "invalid_secret_data"
        }
        _ => "backend_error",
    };
    custody_error(operation, reason)
}

/// In-memory custody for explicitly isolated fixtures. Never selected implicitly.
#[derive(Default)]
pub struct MemoryKeys(Mutex<std::collections::BTreeMap<Uuid, Secret>>);
impl KeyProvider for MemoryKeys {
    fn load(&self, store: Uuid) -> Result<Option<Secret>> {
        let keys = self.0.lock().map_err(|_| key_error())?;
        keys.get(&store)
            .map(|key| Secret::from_bytes(key.expose()))
            .transpose()
    }
    fn insert(&self, store: Uuid, secret: &Secret) -> Result<()> {
        use std::collections::btree_map::Entry;
        match self.0.lock().map_err(|_| key_error())?.entry(store) {
            Entry::Vacant(entry) => {
                entry.insert(Secret::from_bytes(secret.expose())?);
                Ok(())
            }
            Entry::Occupied(_) => Err(key_error()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Catalog,
    Application,
    Output,
    Retrieval,
    Acquisition,
    Source,
    Policy,
    Registry,
    Claim,
    Operation,
    Transaction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub kind: Kind,
    pub owner: Option<Uuid>,
    pub member: Option<Uuid>,
}
impl Identity {
    pub const fn catalog() -> Self {
        Self {
            kind: Kind::Catalog,
            owner: None,
            member: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    store: Uuid,
    identity: Identity,
    generation: u64,
    payload: Box<RawValue>,
    tag: [u8; 32],
}

const BASIC_CONTRACT: &str = "saucepan.checksummed-record";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BasicEnvelope {
    contract: String,
    mode: IntegrityMode,
    store: Uuid,
    identity: Identity,
    generation: u64,
    payload: Box<RawValue>,
    checksum: [u8; 32],
}

/// Derived keys are zeroized when the final store/session owner is dropped.
pub struct Authenticator {
    store: Uuid,
    signing: Option<Zeroizing<[u8; 32]>>,
    credentials: Option<Zeroizing<[u8; 32]>>,
}
impl fmt::Debug for Authenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authenticator")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}
impl Authenticator {
    pub fn basic(store: Uuid) -> Self {
        Self {
            store,
            signing: None,
            credentials: None,
        }
    }
    pub fn mode(&self) -> IntegrityMode {
        if self.signing.is_some() {
            IntegrityMode::Keyring
        } else {
            IntegrityMode::Basic
        }
    }
    pub fn load(store: Uuid, keys: &dyn KeyProvider) -> Result<Self> {
        Self::derive(store, &keys.load(store)?.ok_or_else(key_error)?)
    }
    pub fn provision(store: Uuid, keys: &dyn KeyProvider) -> Result<Self> {
        let secret = Secret::generate()?;
        keys.insert(store, &secret)?;
        Self::derive(store, &secret)
    }
    /// Resume initialization of a reserved identity, before any live metadata exists.
    pub(crate) fn initialize(store: Uuid, keys: &dyn KeyProvider) -> Result<Self> {
        if let Some(secret) = keys.load(store)? {
            Self::derive(store, &secret)
        } else {
            Self::provision(store, keys)
        }
    }
    fn derive(store: Uuid, secret: &Secret) -> Result<Self> {
        Ok(Self {
            store,
            signing: Some(
                crypto::derive_key(
                    secret.expose(),
                    Some(store.as_bytes()),
                    b"saucepan/records/hmac-sha256",
                )
                .map_err(|_| key_error())?,
            ),
            credentials: Some(
                crypto::derive_key(
                    secret.expose(),
                    Some(store.as_bytes()),
                    b"saucepan/credentials/hmac-sha256",
                )
                .map_err(|_| key_error())?,
            ),
        })
    }
    pub fn store(&self) -> Uuid {
        self.store
    }

    pub fn sign(
        &self,
        identity: Identity,
        generation: u64,
        value: &impl Serialize,
    ) -> Result<Vec<u8>> {
        let payload = serde_json::to_string_pretty(value).map_err(|_| invalid_payload())?;
        if payload.len() > MAX_RECORD_BYTES {
            return Err(invalid_payload());
        }
        if self.mode() == IntegrityMode::Basic {
            let checksum = basic_checksum(self.store, identity, generation, payload.as_bytes())?;
            let bytes = serde_json::to_vec_pretty(&BasicEnvelope {
                contract: BASIC_CONTRACT.into(),
                mode: IntegrityMode::Basic,
                store: self.store,
                identity,
                generation,
                payload: RawValue::from_string(payload).map_err(|_| invalid_payload())?,
                checksum,
            })
            .map_err(|_| invalid_payload())?;
            if bytes.len() > MAX_RECORD_BYTES {
                return Err(invalid_payload());
            }
            return Ok(bytes);
        }
        let input = signing_input(self.store, identity, generation, payload.as_bytes())?;
        let tag = crypto::authenticate(
            self.signing.as_ref().ok_or_else(key_error)?.as_ref(),
            input.as_slice(),
        )
        .map_err(Error::from)?;
        let bytes = serde_json::to_vec_pretty(&Envelope {
            store: self.store,
            identity,
            generation,
            payload: RawValue::from_string(payload).map_err(|_| invalid_payload())?,
            tag,
        })
        .map_err(|_| invalid_payload())?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid_payload());
        }
        Ok(bytes)
    }

    /// Verify exact raw payload bytes before typed deserialization. The expected
    /// identity comes from a trusted catalog/path, never from the candidate record.
    pub fn verify<T: DeserializeOwned>(
        &self,
        identity: Identity,
        bytes: &[u8],
    ) -> Result<(u64, T)> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(integrity_error());
        }
        if self.mode() == IntegrityMode::Basic {
            let envelope: BasicEnvelope =
                serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
            if envelope.contract != BASIC_CONTRACT
                || envelope.mode != IntegrityMode::Basic
                || envelope.store != self.store
                || envelope.identity != identity
                || envelope.checksum
                    != basic_checksum(
                        self.store,
                        identity,
                        envelope.generation,
                        envelope.payload.get().as_bytes(),
                    )?
            {
                return Err(integrity_error());
            }
            let value =
                serde_json::from_str(envelope.payload.get()).map_err(|_| integrity_error())?;
            return Ok((envelope.generation, value));
        }
        let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
        if envelope.store != self.store || envelope.identity != identity {
            return Err(integrity_error());
        }
        let input = signing_input(
            envelope.store,
            identity,
            envelope.generation,
            envelope.payload.get().as_bytes(),
        )?;
        crypto::verify(
            self.signing.as_ref().ok_or_else(key_error)?.as_ref(),
            input.as_slice(),
            &envelope.tag,
        )
        .map_err(|_| integrity_error())?;
        let value = serde_json::from_str(envelope.payload.get()).map_err(|_| integrity_error())?;
        Ok((envelope.generation, value))
    }

    /// Only used to select key custody before verifying the root envelope.
    /// No payload or policy may be accepted on the strength of this untrusted hint.
    pub fn store_hint(bytes: &[u8]) -> Result<Uuid> {
        Self::root_hint(bytes).map(|(store, _)| store)
    }
    /// Classify a strict envelope; the caller must still verify before use.
    pub fn root_hint(bytes: &[u8]) -> Result<(Uuid, IntegrityMode)> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(integrity_error());
        }
        #[derive(Deserialize)]
        struct Hint {
            contract: Option<String>,
            mode: Option<String>,
        }
        let hint: Hint = serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
        if hint.contract.is_some() || hint.mode.is_some() {
            if hint.contract.as_deref() != Some(BASIC_CONTRACT)
                || hint.mode.as_deref() != Some("basic")
            {
                return Err(Error::new(
                    ErrorCode::UnsupportedStore,
                    "unsupported metadata integrity contract",
                ));
            }
            let envelope: BasicEnvelope =
                serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
            if envelope.identity != Identity::catalog() {
                return Err(integrity_error());
            }
            return Ok((envelope.store, IntegrityMode::Basic));
        }
        let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
        if envelope.identity != Identity::catalog() {
            return Err(integrity_error());
        }
        Ok((envelope.store, IntegrityMode::Keyring))
    }

    pub fn credential_policy(
        &self,
        app: Uuid,
        generation: u64,
        token: &Credential,
    ) -> Result<CredentialPolicy> {
        let bytes = bearer_input(self.store, app, generation);
        let verifier =
            crypto::authenticate(token.expose(), bytes.as_slice()).map_err(Error::from)?;
        Ok(CredentialPolicy::Bearer {
            generation,
            verifier,
        })
    }

    pub fn check_credential(
        &self,
        app: Uuid,
        policy: &CredentialPolicy,
        token: Option<&Credential>,
    ) -> Result<()> {
        let (generation, verifier) = match policy {
            CredentialPolicy::Disabled => return Ok(()),
            CredentialPolicy::Required {
                generation,
                verifier,
            } => {
                if self.mode() == IntegrityMode::Basic {
                    return Err(integrity_error());
                }
                (generation, verifier)
            }
            CredentialPolicy::Bearer {
                generation,
                verifier,
            } => (generation, verifier),
        };
        let token = token.ok_or_else(|| {
            Error::new(
                ErrorCode::AuthenticationRequired,
                "application credential required",
            )
        })?;
        let verified = match policy {
            CredentialPolicy::Bearer { .. } => crypto::verify(
                token.expose(),
                bearer_input(self.store, app, *generation).as_slice(),
                verifier,
            ),
            CredentialPolicy::Required { .. } => crypto::verify(
                self.credentials.as_ref().ok_or_else(key_error)?.as_ref(),
                credential_input(app, *generation, token).as_slice(),
                verifier,
            ),
            CredentialPolicy::Disabled => unreachable!(),
        };
        verified.map_err(|_| {
            Error::new(
                ErrorCode::AuthenticationFailed,
                "application credential rejected",
            )
        })
    }
}

fn basic_checksum(
    store: Uuid,
    identity: Identity,
    generation: u64,
    payload: &[u8],
) -> Result<[u8; 32]> {
    let header = serde_json::to_vec(&(
        BASIC_CONTRACT,
        IntegrityMode::Basic,
        store,
        identity,
        generation,
    ))
    .map_err(|_| integrity_error())?;
    let mut digest = Sha256::new();
    for field in [
        b"saucepan/checksummed-record".as_slice(),
        header.as_slice(),
        payload,
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    Ok(digest.finalize().into())
}

fn signing_input(
    store: Uuid,
    identity: Identity,
    generation: u64,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let header =
        serde_json::to_vec(&(store, identity, generation)).map_err(|_| integrity_error())?;
    let mut bytes = Vec::with_capacity(header.len() + payload.len() + 64);
    for field in [
        b"saucepan/authenticated-record".as_slice(),
        header.as_slice(),
        payload,
    ] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }
    Ok(bytes)
}
fn credential_input(app: Uuid, generation: u64, token: &Credential) -> Zeroizing<Vec<u8>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(56));
    bytes.extend_from_slice(app.as_bytes());
    bytes.extend_from_slice(&generation.to_be_bytes());
    bytes.extend_from_slice(token.expose());
    bytes
}
fn bearer_input(store: Uuid, app: Uuid, generation: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    for field in [
        b"saucepan/bearer-proof".as_slice(),
        store.as_bytes(),
        app.as_bytes(),
        &generation.to_be_bytes(),
    ] {
        bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
        bytes.extend_from_slice(field);
    }
    bytes
}
pub(crate) fn integrity_error() -> Error {
    Error::new(
        ErrorCode::IntegrityError,
        "metadata integrity verification failed",
    )
}
fn key_error() -> Error {
    Error::new(
        ErrorCode::KeyUnavailable,
        "metadata signing key is unavailable",
    )
}
fn invalid_payload() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "metadata payload is invalid or too large",
    )
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    #[test]
    fn native_errors_preserve_stage_without_exposing_backend_payloads() {
        let error = native_error(
            "load",
            keyring::Error::BadEncoding(b"secret-payload".to_vec()),
        );
        assert_eq!(error.code, ErrorCode::KeyUnavailable);
        assert_eq!(error.details["operation"], "load");
        assert_eq!(error.details["reason"], "invalid_secret_data");
        assert!(!format!("{error:?} {error}").contains("secret-payload"));
        let error = native_error(
            "insert",
            keyring::Error::PlatformFailure(Box::new(std::io::Error::other(
                "secret-platform-data",
            ))),
        );
        assert_eq!(error.details["reason"], "platform_failure");
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("secret-platform-data")
        );
        assert_ne!(
            custody_error("coordinate", "lock_unavailable").message,
            error.message
        );
    }
}
