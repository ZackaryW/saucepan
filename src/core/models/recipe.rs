use super::{EXPORT_VERSION, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Git,
    Http,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLocator {
    pub backend: Backend,
    /// Credentials are never permitted in a persisted recipe locator.
    pub origin: String,
}

/// Canonical content-source identity; revisions and subtrees are separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub schema_version: u32,
    pub backend: Backend,
    pub id: String,
    pub origin: String,
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Revision {
    #[default]
    DefaultBranch,
    Branch(String),
    Tag(String),
    Commit(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub version: u32,
    /// Normalized repository-relative path; "." selects the repository root.
    pub subdirectory: String,
}

impl Default for Export {
    fn default() -> Self {
        Self {
            version: EXPORT_VERSION,
            subdirectory: ".".into(),
        }
    }
}

/// A content recipe carries no grants, retention settings, or verification flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: u32,
    pub source: SourceLocator,
    #[serde(default)]
    pub revision: Revision,
    #[serde(default)]
    pub export: Export,
    #[serde(default)]
    pub manifest: Option<ManifestInput>,
}

impl Recipe {
    pub fn git(origin: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            source: SourceLocator {
                backend: Backend::Git,
                origin: origin.into(),
            },
            revision: Revision::default(),
            export: Export::default(),
            manifest: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestInput {
    pub document: serde_json::Value,
    pub provenance: ManifestProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManifestProvenance {
    Repository {
        path: String,
    },
    Catalog {
        source_id: String,
        revision: String,
        document_digest: String,
    },
    LocalCatalog {
        path: String,
        document_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub path: String,
    pub source_id: String,
    pub origin: String,
    pub commit: String,
    pub subdirectory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LfsObject {
    pub path: String,
    pub oid: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInputs {
    pub source_id: String,
    pub stream_id: String,
    pub subdirectory: String,
    pub commit: String,
    pub dependencies: Vec<Dependency>,
    pub lfs_objects: Vec<LfsObject>,
    pub export_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcquisitionEvidence {
    pub recipe_digest: String,
    pub manifest: Option<ManifestInput>,
    pub policy_revision: u64,
    pub content_rechecked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactDisposition {
    Current,
    Historical,
    Temporary,
}

/// A handle names immutable resolved content, never a moving branch checkout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactHandle {
    pub id: String,
    pub inputs: ResolvedInputs,
    pub archive_digest: String,
    pub tree_digest: String,
    pub archive_size: u64,
    pub disposition: ArtifactDisposition,
    pub evidence: AcquisitionEvidence,
}

/// Shared cache records contain content facts, never application/catalog evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentArtifact {
    pub id: String,
    pub inputs: ResolvedInputs,
    pub archive_digest: String,
    pub tree_digest: String,
    pub archive_size: u64,
}

impl ContentArtifact {
    /// Different tracking/pin streams may name identical output. Their stream
    /// IDs are provenance, not an additional content-identity constraint.
    pub fn same_content(&self, other: &Self) -> bool {
        self.id == other.id
            && self.archive_digest == other.archive_digest
            && self.tree_digest == other.tree_digest
            && self.archive_size == other.archive_size
            && self.inputs.source_id == other.inputs.source_id
            && self.inputs.subdirectory == other.inputs.subdirectory
            && self.inputs.commit == other.inputs.commit
            && self.inputs.dependencies == other.inputs.dependencies
            && self.inputs.lfs_objects == other.inputs.lfs_objects
            && self.inputs.export_version == other.inputs.export_version
    }
}

/// Immutable archive bytes read from an authenticated current/history record.
/// Application recipe and manifest provenance are never taken from shared state.
pub struct ArchiveRead {
    pub artifact: ContentArtifact,
    pub bytes: Vec<u8>,
    pub content_rechecked: bool,
}

impl ArtifactHandle {
    pub fn content_record(&self) -> ContentArtifact {
        ContentArtifact {
            id: self.id.clone(),
            inputs: self.inputs.clone(),
            archive_digest: self.archive_digest.clone(),
            tree_digest: self.tree_digest.clone(),
            archive_size: self.archive_size,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamDescriptor {
    pub source_id: String,
    pub revision: Revision,
    pub export: Export,
}
