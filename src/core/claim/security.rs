//! Authenticated readable records and native secret custody. No encryption or
//! policy decisions live here; callers supply the expected record identity.
use crate::{
    error::{Error, ErrorCode, Result},
    models::claim::security::{Credential, CredentialPolicy},
    utils::crypto,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use std::{fmt, sync::Mutex};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Maximum encoded metadata record size. Content files are not subject to this limit.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const SERVICE: &str = "saucepan.metadata-integrity";

/// Secret storage is explicitly injected for tests. Production uses NativeKeyring.
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
            .map_err(|_| key_error())?
            .delete_credential()
            .map_err(|_| key_error())
    }
}
fn native_coordination() -> Result<(
    std::sync::MutexGuard<'static, ()>,
    crate::utils::lock::LockGuard,
)> {
    let local = NATIVE_KEYS.lock().map_err(|_| key_error())?;
    #[cfg(unix)]
    // SAFETY: geteuid has no arguments or memory safety preconditions.
    let name = format!("saucepan-keyring-{}.lock", unsafe { libc::geteuid() });
    #[cfg(not(unix))]
    let name = "saucepan-keyring.lock";
    let path = std::env::temp_dir().join(name);
    if path.exists() {
        crate::utils::path::plain_metadata(&path).map_err(|_| key_error())?;
    }
    let external = crate::utils::lock::LockGuard::acquire(
        &path,
        crate::utils::lock::LockMode::Exclusive,
        std::time::Duration::from_secs(30),
    )
    .map_err(|_| key_error())?;
    Ok((local, external))
}
impl KeyProvider for NativeKeyring {
    fn load(&self, store: Uuid) -> Result<Option<Secret>> {
        let _guard = native_coordination()?;
        let entry = keyring::Entry::new(SERVICE, &store.to_string()).map_err(|_| key_error())?;
        match entry.get_secret() {
            Ok(bytes) => Secret::from_bytes(&Zeroizing::new(bytes)).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(key_error()),
        }
    }
    fn insert(&self, store: Uuid, secret: &Secret) -> Result<()> {
        let _guard = native_coordination()?;
        // Initialization holds the store's mutation lock. The randomly allocated
        // store UUID is reserved before provisioning and reused after a crash.
        let entry = keyring::Entry::new(SERVICE, &store.to_string()).map_err(|_| key_error())?;
        match entry.get_secret() {
            Err(keyring::Error::NoEntry) => {
                entry.set_secret(secret.expose()).map_err(|_| key_error())?;
                let persisted = Zeroizing::new(entry.get_secret().map_err(|_| key_error())?);
                if persisted.as_slice() != secret.expose() {
                    return Err(key_error());
                }
                Ok(())
            }
            Ok(bytes) => {
                let _discard = Zeroizing::new(bytes);
                Err(key_error())
            }
            Err(_) => Err(key_error()),
        }
    }
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
    Source,
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

/// Derived keys are zeroized when the final store/session owner is dropped.
pub struct Authenticator {
    store: Uuid,
    signing: Zeroizing<[u8; 32]>,
    credentials: Zeroizing<[u8; 32]>,
}
impl fmt::Debug for Authenticator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Authenticator")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}
impl Authenticator {
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
            signing: crypto::derive_key(
                secret.expose(),
                Some(store.as_bytes()),
                b"saucepan/records/hmac-sha256",
            )
            .map_err(|_| key_error())?,
            credentials: crypto::derive_key(
                secret.expose(),
                Some(store.as_bytes()),
                b"saucepan/credentials/hmac-sha256",
            )
            .map_err(|_| key_error())?,
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
        let input = signing_input(self.store, identity, generation, payload.as_bytes())?;
        let tag =
            crypto::authenticate(self.signing.as_ref(), input.as_slice()).map_err(Error::from)?;
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
        crypto::verify(self.signing.as_ref(), input.as_slice(), &envelope.tag)
            .map_err(|_| integrity_error())?;
        let value = serde_json::from_str(envelope.payload.get()).map_err(|_| integrity_error())?;
        Ok((envelope.generation, value))
    }

    /// Only used to select key custody before verifying the root envelope.
    /// No payload or policy may be accepted on the strength of this untrusted hint.
    pub fn store_hint(bytes: &[u8]) -> Result<Uuid> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(integrity_error());
        }
        let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| integrity_error())?;
        if envelope.identity != Identity::catalog() {
            return Err(integrity_error());
        }
        Ok(envelope.store)
    }

    pub fn credential_policy(
        &self,
        app: Uuid,
        generation: u64,
        token: &Credential,
    ) -> Result<CredentialPolicy> {
        let bytes = credential_input(app, generation, token);
        let verifier = crypto::authenticate(self.credentials.as_ref(), bytes.as_slice())
            .map_err(Error::from)?;
        Ok(CredentialPolicy::Required {
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
        let CredentialPolicy::Required {
            generation,
            verifier,
        } = policy
        else {
            return Ok(());
        };
        let token = token.ok_or_else(|| {
            Error::new(
                ErrorCode::AuthenticationRequired,
                "application credential required",
            )
        })?;
        let bytes = credential_input(app, *generation, token);
        crypto::verify(self.credentials.as_ref(), bytes.as_slice(), verifier).map_err(|_| {
            Error::new(
                ErrorCode::AuthenticationFailed,
                "application credential rejected",
            )
        })
    }
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
pub(crate) fn integrity_error() -> Error {
    Error::new(ErrorCode::IntegrityError, "metadata authentication failed")
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
