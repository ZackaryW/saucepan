use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::bucket::{BucketEntry, BucketRegistry};
use crate::error::Conflict;
use crate::sauce::Sauce;
use crate::utils::fs::atomic_write;
use crate::utils::naming::{repo_dir, terminal_component};

// ── types ─────────────────────────────────────────────────────────────────────

/// Which link of the manifest resolution chain supplied an entry's manifest.
///
/// Defaults to `Repository` so entries written before this field existed —
/// which have no `manifest_source` key at all — deserialize as if their
/// manifest came from the repository, matching actual past behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ManifestSource {
    /// The manifest came from the fetched target's own root manifest.
    ///
    /// The default, so an entry written before `manifest_source` existed —
    /// which has no such key at all — deserializes as repository-sourced,
    /// which is what it was.
    #[default]
    Repository,
    /// The manifest came from a registered central index.
    Index {
        /// The registered index target that supplied the manifest.
        index: String,
    },
}

impl ManifestSource {
    /// The manifest came from the target's own repository. This is the only
    /// source today; every current install/update call site uses this
    /// explicitly so behavior is unchanged until the resolution chain exists.
    pub fn repository() -> Self {
        Self::Repository
    }

    /// The manifest came from a registered central index — `index` is the
    /// registered index target that supplied it. Used by the manifest
    /// resolution chain's central-index link (`sources::git::CentralIndexLink`).
    pub fn index(index: impl Into<String>) -> Self {
        Self::Index {
            index: index.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "lowercase")]
pub enum IndexEntry {
    Local {
        path: String,
        sauce: Sauce,
    },
    Github {
        repo: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolved_commit: Option<String>,
        #[serde(default)]
        manifest_source: ManifestSource,
        sauce: Sauce,
    },
    Customgit {
        url: String,
        #[serde(default)]
        manifest_source: ManifestSource,
        sauce: Sauce,
    },
}

impl IndexEntry {
    pub fn sauce(&self) -> &Sauce {
        match self {
            Self::Local { sauce, .. } => sauce,
            Self::Github { sauce, .. } => sauce,
            Self::Customgit { sauce, .. } => sauce,
        }
    }

    pub fn name(&self) -> &str {
        &self.sauce().name
    }

    /// Resolve the on-disk path where this entry's artifact lives.
    ///
    /// For `Local` entries the stored path is returned as-is.
    /// For `Github` and `Customgit` the path is derived from `root` using the
    /// same formula as `fetch_sauce`, so this is always consistent with what
    /// was cloned.
    pub fn artifact_path(&self, root: &Path) -> PathBuf {
        match self {
            Self::Local { path, .. } => PathBuf::from(path),
            Self::Github { repo, .. } => root.join("github").join(repo_dir(repo)),
            Self::Customgit { url, .. } => root
                .join("customgit")
                .join(repo_dir(terminal_component(url))),
        }
    }

    fn same_origin(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local { path: left, .. }, Self::Local { path: right, .. }) => left == right,
            (Self::Github { repo: left, .. }, Self::Github { repo: right, .. }) => left == right,
            (Self::Customgit { url: left, .. }, Self::Customgit { url: right, .. }) => {
                left == right
            }
            _ => false,
        }
    }
}

pub type LocalIndex = Vec<IndexEntry>;

// ── paths ─────────────────────────────────────────────────────────────────────

pub fn saucepan_dir(root: &Path) -> std::path::PathBuf {
    root.join(".saucepan")
}

pub fn index_path(root: &Path) -> std::path::PathBuf {
    saucepan_dir(root).join("index.json")
}

pub fn buckets_path(root: &Path) -> std::path::PathBuf {
    saucepan_dir(root).join("buckets.json")
}

// ── index ─────────────────────────────────────────────────────────────────────

pub fn load_index(root: &Path) -> Result<LocalIndex> {
    let path = index_path(root);
    if !path.exists() {
        return Ok(vec![]);
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&contents).context("invalid index.json")
}

pub fn save_index(root: &Path, index: &LocalIndex) -> Result<()> {
    std::fs::create_dir_all(saucepan_dir(root))?;
    atomic_write(
        &index_path(root),
        serde_json::to_string_pretty(index)?.as_bytes(),
    )
}

/// Insert or replace an entry by sauce name.
/// Returns an error if the same name already exists with a different source type,
/// preventing silent cross-source overwrites.
pub fn upsert(index: &mut LocalIndex, entry: IndexEntry) -> Result<()> {
    if let Some(pos) = index.iter().position(|e| e.name() == entry.name()) {
        if std::mem::discriminant(&index[pos]) != std::mem::discriminant(&entry) {
            return Err(Conflict(format!(
                "sauce '{}' is already installed from a different source type; \
                 run `saucepan <root> uninstall {}` first",
                entry.name(),
                entry.name()
            ))
            .into());
        }
        if !index[pos].same_origin(&entry) {
            return Err(Conflict(format!(
                "sauce '{}' is already installed from a different origin; \
                 run `saucepan <root> uninstall {}` first",
                entry.name(),
                entry.name()
            ))
            .into());
        }
        index[pos] = entry;
    } else {
        index.push(entry);
    }
    Ok(())
}

// ── bucket registry ───────────────────────────────────────────────────────────

pub fn load_registry(root: &Path) -> Result<BucketRegistry> {
    let path = buckets_path(root);
    if !path.exists() {
        return Ok(vec![]);
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&contents).context("invalid buckets.json")
}

pub fn save_registry(root: &Path, registry: &BucketRegistry) -> Result<()> {
    std::fs::create_dir_all(saucepan_dir(root))?;
    atomic_write(
        &buckets_path(root),
        serde_json::to_string_pretty(registry)?.as_bytes(),
    )
}

/// Register a bucket (a local path, `file://` URL, or repository target).
///
/// `reference` pins an explicit ref on a repository-target index (see
/// `BucketEntry::reference`); pass `None` for a local path or `file://` URL,
/// or for a repository target that should always read the latest state.
pub fn registry_add(root: &Path, url: &str, reference: Option<&str>) -> Result<()> {
    let mut reg = load_registry(root)?;
    if reg.iter().any(|e| e.url == url) {
        bail!("bucket already registered: {url}");
    }
    reg.push(BucketEntry {
        url: url.to_string(),
        reference: reference.map(str::to_string),
        resolved_commit: None,
    });
    save_registry(root, &reg)
}

pub fn registry_remove(root: &Path, url: &str) -> Result<()> {
    let mut reg = load_registry(root)?;
    let before = reg.len();
    reg.retain(|e| e.url != url);
    if reg.len() == before {
        bail!("bucket not found: {url}");
    }
    save_registry(root, &reg)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_sauce(name: &str, version: &str) -> Sauce {
        Sauce {
            name: name.to_string(),
            version: version.to_string(),
            description: "test".to_string(),
            extra: Default::default(),
        }
    }

    #[test]
    fn load_index_returns_empty_when_missing() {
        let dir = TempDir::new().unwrap();
        assert!(load_index(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let entry = IndexEntry::Local {
            path: "/fake".to_string(),
            sauce: make_sauce("my-lib", "1.0.0"),
        };
        save_index(dir.path(), &vec![entry]).unwrap();
        let loaded = load_index(dir.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name(), "my-lib");
        assert!(index_path(dir.path()).exists());
    }

    #[test]
    fn save_index_is_atomic() {
        let dir = TempDir::new().unwrap();
        save_index(dir.path(), &vec![]).unwrap();
        assert!(!dir.path().join(".saucepan/index.tmp").exists());
    }

    #[test]
    fn legacy_github_entry_deserializes_without_revision_metadata() {
        let raw = r#"{"source_type":"github","repo":"owner/repo","sauce":{"name":"a","version":"1.0","description":"test"}}"#;
        let entry: IndexEntry = serde_json::from_str(raw).unwrap();

        match entry {
            IndexEntry::Github {
                repo,
                reference,
                resolved_commit,
                manifest_source,
                sauce,
            } => {
                assert_eq!(repo, "owner/repo");
                assert_eq!(sauce.name, "a");
                assert!(reference.is_none());
                assert!(resolved_commit.is_none());
                assert_eq!(manifest_source, ManifestSource::Repository);
            }
            _ => panic!("expected github entry"),
        }
    }

    /// Mirrors `legacy_github_entry_deserializes_without_revision_metadata`:
    /// an entry written before `manifest_source` existed has no such key at
    /// all, and must still deserialize, defaulting the field as if the
    /// manifest came from the repository (the only source that existed then).
    #[test]
    fn legacy_entry_deserializes_without_manifest_source() {
        let raw = r#"{"source_type":"github","repo":"owner/repo","reference":"v1.0","resolved_commit":"abc123","sauce":{"name":"a","version":"1.0","description":"test"}}"#;
        let entry: IndexEntry = serde_json::from_str(raw).unwrap();

        match entry {
            IndexEntry::Github {
                manifest_source, ..
            } => {
                assert_eq!(manifest_source, ManifestSource::Repository);
            }
            _ => panic!("expected github entry"),
        }

        let raw = r#"{"source_type":"customgit","url":"https://example.com/a","sauce":{"name":"a","version":"1.0","description":"test"}}"#;
        let entry: IndexEntry = serde_json::from_str(raw).unwrap();

        match entry {
            IndexEntry::Customgit {
                manifest_source, ..
            } => {
                assert_eq!(manifest_source, ManifestSource::Repository);
            }
            _ => panic!("expected customgit entry"),
        }
    }

    #[test]
    fn manifest_source_repository_and_index_serialize_as_tagged_shape() {
        let raw = serde_json::to_string(&ManifestSource::Repository).unwrap();
        assert_eq!(raw, r#"{"kind":"repository"}"#);

        let raw = serde_json::to_string(&ManifestSource::index("my-index")).unwrap();
        assert_eq!(raw, r#"{"kind":"index","index":"my-index"}"#);
    }

    fn make_github(repo: &str, name: &str, version: &str) -> IndexEntry {
        IndexEntry::Github {
            repo: repo.to_string(),
            reference: None,
            resolved_commit: None,
            manifest_source: ManifestSource::repository(),
            sauce: make_sauce(name, version),
        }
    }

    #[test]
    fn absent_github_revision_metadata_is_omitted_when_serialized() {
        let entry = IndexEntry::Github {
            repo: "owner/repo".to_string(),
            reference: None,
            resolved_commit: None,
            manifest_source: ManifestSource::repository(),
            sauce: make_sauce("a", "1.0"),
        };

        let raw = serde_json::to_string(&entry).unwrap();
        assert!(!raw.contains("reference"));
        assert!(!raw.contains("resolved_commit"));
    }

    #[test]
    fn upsert_adds_new_entry() {
        let mut idx = vec![];
        upsert(
            &mut idx,
            IndexEntry::Local {
                path: "/a".to_string(),
                sauce: make_sauce("a", "1.0"),
            },
        )
        .unwrap();
        assert_eq!(idx.len(), 1);
    }

    #[test]
    fn upsert_replaces_same_source_type() {
        let mut idx = vec![];
        upsert(&mut idx, make_github("r", "a", "1.0")).unwrap();
        upsert(&mut idx, make_github("r", "a", "2.0")).unwrap();
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].sauce().version, "2.0");
    }

    #[test]
    fn upsert_errors_on_source_type_conflict() {
        let mut idx = vec![];
        upsert(
            &mut idx,
            IndexEntry::Local {
                path: "/a".to_string(),
                sauce: make_sauce("a", "1.0"),
            },
        )
        .unwrap();
        let err = upsert(&mut idx, make_github("r", "a", "2.0"));
        assert!(err.is_err());
        assert!(
            err.unwrap_err()
                .to_string()
                .contains("different source type")
        );
        assert!(matches!(&idx[0], IndexEntry::Local { path, .. } if path == "/a"));
        assert_eq!(idx[0].sauce().version, "1.0");
    }

    #[test]
    fn upsert_errors_when_different_github_origin_reuses_name() {
        let mut idx = vec![make_github("owner/one", "a", "1.0")];

        let err = upsert(&mut idx, make_github("owner/two", "a", "2.0")).unwrap_err();

        assert!(err.to_string().contains("different origin"));
        assert!(matches!(&idx[0], IndexEntry::Github { repo, .. } if repo == "owner/one"));
        assert_eq!(idx[0].sauce().version, "1.0");
    }

    #[test]
    fn upsert_errors_when_different_customgit_origin_reuses_name() {
        let mut idx = vec![IndexEntry::Customgit {
            url: "https://one.example/a".to_string(),
            manifest_source: ManifestSource::repository(),
            sauce: make_sauce("a", "1.0"),
        }];
        let incoming = IndexEntry::Customgit {
            url: "https://two.example/a".to_string(),
            manifest_source: ManifestSource::repository(),
            sauce: make_sauce("a", "2.0"),
        };

        let err = upsert(&mut idx, incoming).unwrap_err();

        assert!(err.to_string().contains("different origin"));
        assert!(matches!(
            &idx[0],
            IndexEntry::Customgit { url, .. } if url == "https://one.example/a"
        ));
        assert_eq!(idx[0].sauce().version, "1.0");
    }

    #[test]
    fn upsert_preserves_other_entries() {
        let mut idx = vec![];
        for (path, name, version) in [("/a", "a", "1.0"), ("/b", "b", "1.0"), ("/a", "a", "2.0")] {
            upsert(
                &mut idx,
                IndexEntry::Local {
                    path: path.to_string(),
                    sauce: make_sauce(name, version),
                },
            )
            .unwrap();
        }
        assert_eq!(idx.len(), 2);
        assert_eq!(idx[0].sauce().version, "2.0");
        assert_eq!(idx[1].name(), "b");
    }

    #[test]
    fn saucepan_dir_is_dot_saucepan() {
        let dir = TempDir::new().unwrap();
        assert_eq!(saucepan_dir(dir.path()), dir.path().join(".saucepan"));
    }

    #[test]
    fn registry_add_and_load() {
        let dir = TempDir::new().unwrap();
        registry_add(dir.path(), "https://example.com/b.json", None).unwrap();
        let reg = load_registry(dir.path()).unwrap();
        assert_eq!(reg.len(), 1);
        assert_eq!(reg[0].url, "https://example.com/b.json");
    }

    #[test]
    fn registry_add_duplicate_errors() {
        let dir = TempDir::new().unwrap();
        registry_add(dir.path(), "https://example.com/b.json", None).unwrap();
        assert!(registry_add(dir.path(), "https://example.com/b.json", None).is_err());
    }

    #[test]
    fn registry_remove_entry() {
        let dir = TempDir::new().unwrap();
        registry_add(dir.path(), "https://example.com/b.json", None).unwrap();
        registry_remove(dir.path(), "https://example.com/b.json").unwrap();
        assert!(load_registry(dir.path()).unwrap().is_empty());
    }

    /// Registration by repository target: a bare `owner/repo`-shaped string
    /// (or any other git-clonable target) registers exactly like a local path
    /// or `file://` URL does — the registry does not need to know which kind
    /// of target it is, only `fetch_bucket` (bucket.rs) does, at read time.
    #[test]
    fn registry_add_accepts_a_repository_target() {
        let dir = TempDir::new().unwrap();
        registry_add(dir.path(), "owner/central-index", None).unwrap();
        let reg = load_registry(dir.path()).unwrap();
        assert_eq!(reg[0].url, "owner/central-index");
        assert!(reg[0].reference.is_none());
    }

    /// An index entry registered with an explicit ref records that ref, so a
    /// later fetch can resolve and pin to it (see
    /// bucket::tests::fetch_bucket_repository_target_respects_pinned_reference
    /// for the fetch-time behavior this enables).
    #[test]
    fn registry_add_records_an_explicit_reference() {
        let dir = TempDir::new().unwrap();
        registry_add(dir.path(), "owner/central-index", Some("v1.2.0")).unwrap();
        let reg = load_registry(dir.path()).unwrap();
        assert_eq!(reg[0].reference.as_deref(), Some("v1.2.0"));
        // Not yet resolved until the index is actually fetched.
        assert!(reg[0].resolved_commit.is_none());
    }

    #[test]
    fn registry_entry_without_reference_omits_it_when_serialized() {
        let dir = TempDir::new().unwrap();
        registry_add(dir.path(), "https://example.com/b.json", None).unwrap();
        let raw = std::fs::read_to_string(buckets_path(dir.path())).unwrap();
        assert!(!raw.contains("reference"));
        assert!(!raw.contains("resolved_commit"));
    }
}
