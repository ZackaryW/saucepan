use super::{ArtifactHandle, ContentArtifact, ResolvedInputs, SCHEMA_VERSION, StreamDescriptor};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Inspect,
    Setup,
    Update,
    Mirror,
    Remove,
    Manage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub source_id: Option<String>,
    pub selection: String,
    pub actions: BTreeSet<Action>,
    pub destinations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationMode {
    Ordinary,
    Authoritative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRegistration {
    pub root: String,
    pub mode: ApplicationMode,
    pub grants: Vec<Grant>,
    pub require_verification: bool,
}

/// Sensitive bearer material returned only by an explicit management operation.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationCredential {
    pub registration_id: String,
    pub marker: Option<Marker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub id: String,
    pub root: String,
    pub revision: u64,
    pub data_generation: u64,
    pub mode: ApplicationMode,
    pub revoked: bool,
    pub grants: Vec<Grant>,
    pub require_verification: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachePolicy {
    pub revision: u64,
    pub enabled: bool,
    pub history_limit: usize,
    pub verify_by_default: bool,
    pub require_verification: bool,
}
impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            revision: 1,
            enabled: true,
            history_limit: 5,
            verify_by_default: false,
            require_verification: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRequest {
    pub content: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePolicy {
    pub revision: u64,
    pub retain: bool,
    pub history_limit: usize,
    pub verify_content: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Current {
    pub recipe: StreamDescriptor,
    pub artifact: ContentArtifact,
    pub generation: u64,
    pub last_used: u64,
    pub archive_retained: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Historical {
    pub artifact: ContentArtifact,
    pub created: u64,
    pub last_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIndex {
    pub schema_version: u32,
    pub id: String,
    pub origin: String,
    pub generation: u64,
    pub sequence: u64,
    pub policy: CachePolicy,
    pub current: BTreeMap<String, Current>,
    pub history: BTreeMap<String, Historical>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitBinding {
    pub id: String,
    pub app_id: String,
    pub name: String,
    pub recipe: super::Recipe,
    pub artifact: ArtifactHandle,
    pub manifest: serde_json::Value,
    pub materialization: Option<OwnedDirectory>,
    pub mirrors: Vec<MirrorBinding>,
}

/// Opaque equality tokens for a caller's scope and binding data. Neither value
/// exposes the central store's counters or activity in other applications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationGeneration {
    pub scope: String,
    pub data: String,
}

/// Installed metadata without private storage bookkeeping or mirror locations.
/// Resolve directories through the separately checked path APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitView {
    pub id: String,
    pub name: String,
    pub recipe: super::Recipe,
    pub artifact: ArtifactHandle,
    pub manifest: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationView {
    pub schema_version: u32,
    pub generation: ApplicationGeneration,
    pub units: Vec<UnitView>,
}

/// An explicit materialization has ownership without an installed manifest name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationBinding {
    pub id: String,
    pub app_id: String,
    pub recipe: super::Recipe,
    pub artifact: ArtifactHandle,
    pub directory: OwnedDirectory,
    pub mirrors: Vec<MirrorBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MirrorBinding {
    pub artifact: ContentArtifact,
    pub directory: OwnedDirectory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedArtifact {
    pub id: String,
    pub path: String,
    pub artifact: ArtifactHandle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BindingTarget {
    Unit { name: String },
    Materialization { id: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedDirectory {
    pub path: String,
    pub tree_digest: String,
    /// Application-local publication generation; never exposes store activity.
    pub generation: u64,
    /// Creation metadata, authenticated with the binding. Never replaces the
    /// artifact's expected tree digest when optional checking is disabled.
    pub entries: BTreeMap<String, MaterializedEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedEntry {
    pub mode: u32,
    pub size: u64,
    pub digest: String,
    pub implicit_directory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogBinding {
    pub target: String,
    pub requested_ref: Option<String>,
    pub resolved_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRegistration {
    pub version: String,
    pub digest: String,
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub store_schema_min: u32,
    pub store_schema_max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CentralIndex {
    pub schema_version: u32,
    pub store_id: String,
    pub key_generation: u64,
    pub generation: u64,
    pub applications: BTreeMap<String, Registration>,
    /// Source identity -> committed immutable source-index generation.
    pub sources: BTreeMap<String, u64>,
    pub units: BTreeMap<String, UnitBinding>,
    #[serde(default)]
    pub materializations: BTreeMap<String, MaterializationBinding>,
    pub catalogs: BTreeMap<String, Vec<CatalogBinding>>,
    pub runtime: Option<RuntimeRegistration>,
}
impl CentralIndex {
    pub fn empty(store_id: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            store_id,
            key_generation: 1,
            generation: 0,
            applications: BTreeMap::new(),
            sources: BTreeMap::new(),
            units: BTreeMap::new(),
            materializations: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            runtime: None,
        }
    }
}

/// Non-secret authenticated credential fields. Grants stay in CentralIndex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkerClaims {
    pub schema_version: u32,
    pub store_id: String,
    pub registration_id: String,
    pub registration_revision: u64,
    pub enrolled_root: String,
    pub nonce: String,
    pub key_generation: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub claims: MarkerClaims,
    pub mac: String,
}

/// Bounds and authenticates document identity before any JSON payload is parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentIdentity {
    pub schema_version: u32,
    pub store_id: String,
    pub kind: String,
    pub document_id: String,
    pub key_generation: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedEnvelope {
    pub identity: DocumentIdentity,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransactionRecord {
    pub schema_version: u32,
    pub id: String,
    pub expected_central_generation: u64,
    pub source_generations: BTreeMap<String, u64>,
    pub owned_staging: Vec<String>,
    pub new_source_generations: BTreeMap<String, u64>,
    pub pending_inputs: Vec<ResolvedInputs>,
}
