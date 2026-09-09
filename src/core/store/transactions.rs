//! Immutable encrypted generations and one authenticated central commit point.
//! All methods require the central lock. No network or caller callbacks run
//! between journal publication and commit. Unselected files are never authority.
use super::mirrors::{self, MirrorChange};
use super::{
    crypto::{Keys, MAX_ENVELOPE_BYTES},
    layout::Layout,
    locking::Lock,
};
use crate::core::materialization;
use crate::{core::models::*, utils};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const INDEX: &str = "index.json.enc";
const JOURNAL: &str = "transactions/pending.json.enc";

pub(super) struct Generations<'a> {
    pub layout: &'a Layout,
    pub keys: &'a Keys,
    pub store_id: &'a str,
    pub key_generation: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Publication {
    schema_version: u32,
    id: String,
    before_digest: String,
    after_digest: String,
    before_generation: u64,
    after_generation: u64,
    files: Vec<OwnedFile>,
    retired: Vec<OwnedFile>,
    #[serde(default)]
    directories: Vec<DirectoryPublication>,
    #[serde(default)]
    retired_directories: Vec<OwnedDirectory>,
    #[serde(default)]
    mirrors: Vec<MirrorChange>,
}

struct Trees {
    directories: Vec<DirectoryPublication>,
    mirrors: Vec<MirrorChange>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectoryPublication {
    pub staged: String,
    pub owned: OwnedDirectory,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedFile {
    path: String,
    digest: String,
}

pub(super) struct Snapshot {
    pub central: CentralIndex,
    pub sources: BTreeMap<String, SourceIndex>,
}

impl Generations<'_> {
    fn identity(&self, kind: &str, id: &str) -> DocumentIdentity {
        DocumentIdentity {
            schema_version: SCHEMA_VERSION,
            store_id: self.store_id.into(),
            kind: kind.into(),
            document_id: id.into(),
            key_generation: self.key_generation,
        }
    }
    fn encode(&self, kind: &str, id: &str, value: &impl Serialize) -> Result<Vec<u8>> {
        let bytes = Zeroizing::new(
            serde_json::to_vec(value)
                .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode store document"))?,
        );
        self.keys.seal(&self.identity(kind, id), &bytes)
    }
    fn decode<T: DeserializeOwned>(&self, kind: &str, id: &str, bytes: &[u8]) -> Result<T> {
        let plain = self.keys.open(&self.identity(kind, id), bytes)?;
        serde_json::from_slice(&plain)
            .map_err(|_| Error::new(ErrorKind::Integrity, "invalid authenticated store document"))
    }
    pub(super) fn initialize(&self, central: &CentralIndex) -> Result<()> {
        self.layout
            .create_directory(Path::new("transactions/generations"))?;
        self.validate_central(central)?;
        let bytes = self.encode("central", "index", central)?;
        self.layout
            .create_file(Path::new(&central_path(central.generation)), &bytes)?;
        self.layout.create_file(Path::new(INDEX), &bytes)
    }
    fn validate_central(&self, index: &CentralIndex) -> Result<()> {
        if index.schema_version != SCHEMA_VERSION {
            return Err(Error::new(
                ErrorKind::Compatibility,
                "unsupported authority schema",
            ));
        }
        if index.store_id != self.store_id || index.key_generation != self.key_generation {
            return Err(integrity("authority enrollment mismatch"));
        }
        for (id, generation) in &index.sources {
            source_id(id)?;
            if *generation == 0 || *generation > index.generation {
                return Err(integrity("invalid committed source generation"));
            }
        }
        Ok(())
    }
    fn central(&self) -> Result<(CentralIndex, Vec<u8>)> {
        if !self.layout.path(Path::new(INDEX))?.exists() {
            return Err(Error::new(
                ErrorKind::Authority,
                "established authority index is missing; explicit recovery required",
            ));
        }
        let bytes = self.layout.read(Path::new(INDEX), MAX_ENVELOPE_BYTES)?;
        let central: CentralIndex = self.decode("central", "index", &bytes)?;
        self.validate_central(&central)?;
        let immutable = self.layout.read(
            Path::new(&central_path(central.generation)),
            MAX_ENVELOPE_BYTES,
        )?;
        if immutable != bytes {
            return Err(integrity(
                "central generation does not match its commit reference",
            ));
        }
        Ok((central, bytes))
    }
    pub(super) fn read(&self) -> Result<Snapshot> {
        let (central, _) = self.central()?;
        let mut sources = BTreeMap::new();
        for (id, generation) in &central.sources {
            let path = source_path(id, *generation);
            let bytes = self.layout.read(Path::new(&path), MAX_ENVELOPE_BYTES)?;
            let source: SourceIndex =
                self.decode("source", &format!("{id}/{generation}"), &bytes)?;
            if source.schema_version != SCHEMA_VERSION {
                return Err(Error::new(
                    ErrorKind::Compatibility,
                    "unsupported source schema",
                ));
            }
            if source.id != *id || source.generation != *generation {
                return Err(integrity("source generation identity mismatch"));
            }
            sources.insert(id.clone(), source);
        }
        Ok(Snapshot { central, sources })
    }
    /// A fully prepared next central record and changed source records. Source
    /// generations are allocated from the central sequence, never reused after
    /// a successful commit. The caller holds the ordered resource/central locks.
    pub(super) fn commit(
        &self,
        expected: u64,
        central: CentralIndex,
        changed: Vec<SourceIndex>,
    ) -> Result<()> {
        self.commit_steps(expected, central, changed, |_| Ok(()))
    }
    pub(super) fn commit_archives(
        &self,
        expected: u64,
        central: CentralIndex,
        changed: Vec<SourceIndex>,
        archives: Vec<(String, Vec<u8>)>,
    ) -> Result<()> {
        self.publish(expected, central, changed, archives, vec![], |_| Ok(()))
    }
    pub(super) fn commit_directory(
        &self,
        expected: u64,
        central: CentralIndex,
        directory: DirectoryPublication,
    ) -> Result<()> {
        self.publish(expected, central, vec![], vec![], vec![directory], |_| {
            Ok(())
        })
    }
    pub(super) fn commit_materialization(
        &self,
        expected: u64,
        central: CentralIndex,
        source: SourceIndex,
        directory: DirectoryPublication,
    ) -> Result<()> {
        self.publish(
            expected,
            central,
            vec![source],
            vec![],
            vec![directory],
            |_| Ok(()),
        )
    }
    fn commit_steps(
        &self,
        expected: u64,
        central: CentralIndex,
        changed: Vec<SourceIndex>,
        checkpoint: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        self.publish(expected, central, changed, vec![], vec![], checkpoint)
    }
    pub(super) fn commit_mirror(
        &self,
        expected: u64,
        central: CentralIndex,
        mirror: MirrorChange,
    ) -> Result<()> {
        self.publish_trees(
            expected,
            central,
            vec![],
            vec![],
            Trees {
                directories: vec![],
                mirrors: vec![mirror],
            },
            |_| Ok(()),
        )
    }
    fn publish(
        &self,
        expected: u64,
        central: CentralIndex,
        changed: Vec<SourceIndex>,
        archives: Vec<(String, Vec<u8>)>,
        directories: Vec<DirectoryPublication>,
        checkpoint: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        self.publish_trees(
            expected,
            central,
            changed,
            archives,
            Trees {
                directories,
                mirrors: vec![],
            },
            checkpoint,
        )
    }
    fn publish_trees(
        &self,
        expected: u64,
        mut central: CentralIndex,
        mut changed: Vec<SourceIndex>,
        archives: Vec<(String, Vec<u8>)>,
        trees: Trees,
        mut checkpoint: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        self.recover()?;
        let previous = self.read()?;
        if previous.central.generation != expected {
            return Err(Error::new(
                ErrorKind::Busy,
                "store changed during preparation; retry from current authority",
            ));
        }
        let next = expected
            .checked_add(1)
            .ok_or_else(|| integrity("generation exhausted"))?;
        central.generation = next;
        let Trees {
            directories,
            mirrors,
        } = trees;
        let mirrors = mirrors::prepare(self.layout, &previous.central, &central, mirrors)?;
        let old_trees = unit_directories(&previous.central)?;
        let new_trees = unit_directories(&central)?;
        let retired_directories: Vec<_> = old_trees
            .iter()
            .filter(|(path, _)| !new_trees.contains_key(*path))
            .map(|(_, tree)| tree.clone())
            .collect();
        let mut supplied_trees = BTreeMap::new();
        for directory in &directories {
            validate_directory_publication(directory)?;
            if directory.owned.generation == 0
                || new_trees.get(&directory.owned.path) != Some(&directory.owned)
                || supplied_trees
                    .insert(directory.owned.path.clone(), &directory.owned)
                    .is_some()
            {
                return Err(integrity(
                    "directory publication does not match selected binding",
                ));
            }
            if self.layout.path(Path::new(&directory.owned.path))?.exists() {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "materialization destination already exists",
                ));
            }
            materialization::check(
                &self.layout.path(Path::new(&directory.staged))?,
                &directory.owned.entries,
                false,
            )?;
        }
        for (path, tree) in &new_trees {
            if old_trees.get(path) != Some(tree) && supplied_trees.get(path).copied() != Some(tree)
            {
                return Err(integrity("unbacked materialization binding edit"));
            }
        }
        for tree in &retired_directories {
            materialization::check(
                &self.layout.path(Path::new(&tree.path))?,
                &tree.entries,
                true,
            )
            .map_err(|_| {
                Error::new(
                    ErrorKind::Conflict,
                    "previous materialization was modified; preserve it before replacement",
                )
            })?;
        }
        // Every source-map edit must be backed by a supplied immutable source;
        // deleting a source requires the separate source-removal transaction.
        if central.sources != previous.central.sources {
            return Err(integrity("unbacked source generation edit"));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut files = Vec::new();
        let old_central_path = central_path(expected);
        let mut retired = vec![OwnedFile {
            digest: digest(
                &self
                    .layout
                    .read(Path::new(&old_central_path), MAX_ENVELOPE_BYTES)?,
            ),
            path: old_central_path,
        }];
        for source in &mut changed {
            source_id(&source.id)?;
            let old_generation = previous
                .sources
                .get(&source.id)
                .map_or(0, |old| old.generation);
            if source.generation != old_generation {
                return Err(Error::new(
                    ErrorKind::Busy,
                    "source changed during preparation",
                ));
            }
            if !seen.insert(source.id.clone()) || source.schema_version != SCHEMA_VERSION {
                return Err(integrity("invalid source publication set"));
            }
            let new_archives = source_archives(source)?;
            if let Some(old) = previous.sources.get(&source.id) {
                for (path, expected_digest) in source_archives(old)? {
                    if !new_archives.contains_key(&path) {
                        retired.push(OwnedFile {
                            path,
                            digest: expected_digest,
                        });
                    }
                }
            }
            if old_generation != 0 {
                let path = source_path(&source.id, old_generation);
                retired.push(OwnedFile {
                    digest: digest(&self.layout.read(Path::new(&path), MAX_ENVELOPE_BYTES)?),
                    path,
                });
            }
            source.generation = next;
            central.sources.insert(source.id.clone(), next);
            files.push((
                source_path(&source.id, next),
                self.encode("source", &format!("{}/{next}", source.id), source)?,
            ));
        }
        self.validate_central(&central)?;
        let central_bytes = self.encode("central", "index", &central)?;
        files.push((central_path(next), central_bytes.clone()));
        let expected_archives: BTreeMap<_, _> = changed
            .iter()
            .map(source_archives)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        let mut supplied = std::collections::BTreeSet::new();
        for (path, bytes) in archives {
            if !supplied.insert(path.clone())
                || expected_archives.get(&path) != Some(&digest(&bytes))
            {
                return Err(integrity(
                    "archive publication does not match selected content",
                ));
            }
            if self.layout.path(Path::new(&path))?.exists() {
                if digest(&self.layout.read(Path::new(&path), file_limit(&path))?) != digest(&bytes)
                {
                    return Err(integrity(
                        "existing archive does not match committed content",
                    ));
                }
            } else {
                files.push((path, bytes));
            }
        }
        for path in expected_archives.keys() {
            if !files.iter().any(|(file, _)| file == path)
                && !self.layout.path(Path::new(path))?.is_file()
            {
                return Err(integrity("selected archive is missing"));
            }
        }
        for file in &retired {
            if digest(
                &self
                    .layout
                    .read(Path::new(&file.path), file_limit(&file.path))?,
            ) != file.digest
            {
                return Err(integrity("outgoing committed file was modified"));
            }
        }
        // Retiring bytes cannot be selected while a reader still uses them.
        // Acquire before the journal or commit point, and in stable path order.
        let archive_locks = self.cleanup_leases(&retired)?;
        // Preflight every destination before the journal can claim ownership.
        for (path, _) in &files {
            if self.layout.path(Path::new(path))?.exists() {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "immutable publication destination already exists",
                ));
            }
        }
        for source in &changed {
            let path = format!("sources/{}", source.id);
            if !self.layout.path(Path::new(&path))?.exists() {
                self.layout.create_directory(Path::new(&path))?;
            }
        }
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce)
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        let before = self.layout.read(Path::new(INDEX), MAX_ENVELOPE_BYTES)?;
        let journal = Publication {
            schema_version: SCHEMA_VERSION,
            id: utils::hex(&nonce),
            before_digest: digest(&before),
            after_digest: digest(&central_bytes),
            before_generation: expected,
            after_generation: next,
            files: files
                .iter()
                .map(|(path, bytes)| OwnedFile {
                    path: path.clone(),
                    digest: digest(bytes),
                })
                .collect(),
            retired,
            directories,
            retired_directories,
            mirrors,
        };
        self.layout.create_file(
            Path::new(JOURNAL),
            &self.encode("transaction", "pending", &journal)?,
        )?;
        checkpoint(0)?;
        for (step, (path, bytes)) in files.iter().enumerate() {
            self.layout.create_file(Path::new(path), bytes)?;
            checkpoint(step + 1)?;
        }
        for (step, directory) in journal.directories.iter().enumerate() {
            let staged = self.layout.path(Path::new(&directory.staged))?;
            let destination = self.layout.path(Path::new(&directory.owned.path))?;
            if destination.exists() {
                return Err(integrity(
                    "materialization publication destination became occupied",
                ));
            }
            std::fs::rename(staged, &destination)
                .map_err(|_| integrity("cannot publish staged materialization"))?;
            materialization::sync_dir(
                destination
                    .parent()
                    .expect("validated materialization parent"),
            )?;
            materialization::sync_dir(&self.layout.path(Path::new("transactions"))?)?;
            checkpoint(files.len() + step + 1)?;
        }
        let tree_steps = files.len() + journal.directories.len();
        for (index, mirror) in journal.mirrors.iter().enumerate() {
            mirror.publish(self.layout, |step| {
                checkpoint(tree_steps + index * 2 + step + 1)
            })?;
        }
        let tree_steps = tree_steps + journal.mirrors.len() * 2;
        // The ONLY selection point. All selected immutable bytes were flushed.
        self.layout.replace(Path::new(INDEX), &central_bytes)?;
        checkpoint(tree_steps + 1)?;
        drop(archive_locks);
        self.recover()?;
        checkpoint(tree_steps + 2)
    }
    /// Recover only what this authenticated journal owns. Never select staged
    /// files, infer permissions, recurse over a directory, or overwrite a clash.
    pub(super) fn recover(&self) -> Result<()> {
        self.recover_steps(|_| Ok(()))
    }
    fn recover_steps(&self, mut checkpoint: impl FnMut(usize) -> Result<()>) -> Result<()> {
        if !self.layout.path(Path::new(JOURNAL))?.exists() {
            return Ok(());
        }
        let journal: Publication = self.decode(
            "transaction",
            "pending",
            &self.layout.read(Path::new(JOURNAL), MAX_ENVELOPE_BYTES)?,
        )?;
        if journal.schema_version != SCHEMA_VERSION
            || journal.id.len() != 32
            || journal.after_generation
                != journal
                    .before_generation
                    .checked_add(1)
                    .ok_or_else(|| integrity("invalid journal generation"))?
        {
            return Err(integrity("invalid transaction journal"));
        }
        let (central, bytes) = self.central()?;
        let selected = if digest(&bytes) == journal.after_digest
            && central.generation == journal.after_generation
        {
            true
        } else if digest(&bytes) == journal.before_digest
            && central.generation == journal.before_generation
        {
            false
        } else {
            return Err(integrity("transaction does not match committed authority"));
        };
        // Validate the entire cleanup set before the first removal.
        for mirror in &journal.mirrors {
            mirror.validate_recovery(self.layout, selected, &central)?;
        }
        let selected_trees = unit_directories(&central)?;
        for directory in &journal.directories {
            validate_directory_publication(directory)?;
            if selected && selected_trees.get(&directory.owned.path) != Some(&directory.owned) {
                return Err(integrity(
                    "journal directory does not match selected binding",
                ));
            }
            if !selected && selected_trees.contains_key(&directory.owned.path) {
                return Err(integrity("cannot remove a selected materialization"));
            }
            materialization::check(
                &self.layout.path(Path::new(&directory.owned.path))?,
                &directory.owned.entries,
                !selected,
            )?;
            materialization::check(
                &self.layout.path(Path::new(&directory.staged))?,
                &directory.owned.entries,
                true,
            )?;
        }
        for directory in &journal.retired_directories {
            validate_materialization_path(&directory.path)?;
            if selected {
                if selected_trees.contains_key(&directory.path) {
                    return Err(integrity("cannot retire a selected materialization"));
                }
                materialization::check(
                    &self.layout.path(Path::new(&directory.path))?,
                    &directory.entries,
                    true,
                )?;
            }
        }
        for file in &journal.files {
            validate_owned_path(&file.path, journal.after_generation)?;
            let path = self.layout.path(Path::new(&file.path))?;
            if path.exists() {
                if digest(
                    &self
                        .layout
                        .read(Path::new(&file.path), file_limit(&file.path))?,
                ) != file.digest
                {
                    return Err(integrity(
                        "journal-owned file was modified; explicit repair required",
                    ));
                }
            } else if selected {
                return Err(integrity("committed transaction file is missing"));
            }
        }
        if selected {
            let snapshot = self.read()?; // Authenticate every selected source before completing.
            for file in &journal.retired {
                if file.path.ends_with(".zip") {
                    validate_archive_path(&file.path)?;
                    for source in snapshot.sources.values() {
                        if source_archives(source)?.contains_key(&file.path) {
                            return Err(integrity("cannot retire a selected archive"));
                        }
                    }
                } else {
                    validate_retired_path(&file.path, &central, journal.before_generation)?;
                }
                if self.layout.path(Path::new(&file.path))?.exists()
                    && digest(
                        &self
                            .layout
                            .read(Path::new(&file.path), file_limit(&file.path))?,
                    ) != file.digest
                {
                    return Err(integrity(
                        "retired index was modified; explicit repair required",
                    ));
                }
            }
        }
        let cleanup = if selected {
            &journal.retired
        } else {
            &journal.files
        };
        let _archive_locks = self.cleanup_leases(cleanup)?;
        for mirror in &journal.mirrors {
            mirror.recover(self.layout, selected, &central)?;
        }
        for directory in &journal.directories {
            materialization::cleanup(
                &self.layout.path(Path::new(&directory.staged))?,
                &directory.owned.entries,
            )?;
            if !selected {
                materialization::cleanup(
                    &self.layout.path(Path::new(&directory.owned.path))?,
                    &directory.owned.entries,
                )?;
            }
        }
        if selected {
            for directory in &journal.retired_directories {
                materialization::cleanup(
                    &self.layout.path(Path::new(&directory.path))?,
                    &directory.entries,
                )?;
            }
        }
        for (step, file) in cleanup.iter().enumerate() {
            self.layout.remove_file(Path::new(&file.path))?;
            checkpoint(step)?;
        }
        self.layout.remove_file(Path::new(JOURNAL))
    }
    fn cleanup_leases(&self, files: &[OwnedFile]) -> Result<Vec<Lock>> {
        let mut paths: Vec<_> = files
            .iter()
            .filter(|file| file.path.ends_with(".zip"))
            .map(|file| file.path.as_str())
            .collect();
        paths.sort_unstable();
        paths.dedup();
        let deadline = Instant::now() + Duration::from_secs(5);
        paths
            .into_iter()
            .map(|path| {
                validate_archive_path(path)?;
                let parts: Vec<_> = path.split('/').collect();
                let id = parts[3]
                    .strip_suffix(".zip")
                    .expect("validated archive suffix");
                Lock::acquire(self.layout, &format!("archive-{}-{id}", parts[1]), deadline)
            })
            .collect()
    }
}
fn unit_directories(central: &CentralIndex) -> Result<BTreeMap<String, OwnedDirectory>> {
    let mut result = BTreeMap::new();
    for tree in central
        .units
        .values()
        .filter_map(|unit| unit.materialization.as_ref())
        .chain(
            central
                .materializations
                .values()
                .map(|binding| &binding.directory),
        )
    {
        validate_materialization_path(&tree.path)?;
        if result.insert(tree.path.clone(), tree.clone()).is_some() {
            return Err(integrity("materialization is shared by multiple bindings"));
        }
    }
    Ok(result)
}
fn validate_materialization_path(path: &str) -> Result<()> {
    source_id(
        path.strip_prefix("materializations/")
            .ok_or_else(|| integrity("unowned materialization path"))?,
    )
}
fn validate_directory_publication(directory: &DirectoryPublication) -> Result<()> {
    validate_materialization_path(&directory.owned.path)?;
    let suffix = directory
        .staged
        .strip_prefix("transactions/materialize-")
        .ok_or_else(|| integrity("unowned materialization staging"))?;
    if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(integrity("unowned materialization staging"));
    }
    Ok(())
}
fn digest(bytes: &[u8]) -> String {
    utils::hex(&Sha256::digest(bytes))
}
fn integrity(message: &str) -> Error {
    Error::new(ErrorKind::Integrity, message)
}
fn source_id(id: &str) -> Result<()> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(integrity("invalid canonical source identity"));
    }
    Ok(())
}
fn central_path(generation: u64) -> String {
    format!("transactions/generations/central-{generation}.json.enc")
}
fn source_path(id: &str, generation: u64) -> String {
    format!("sources/{id}/index-{generation}.json.enc")
}
fn validate_owned_path(path: &str, generation: u64) -> Result<()> {
    if path.ends_with(".zip") {
        return validate_archive_path(path);
    }
    if path == central_path(generation) {
        return Ok(());
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() == 3
        && parts[0] == "sources"
        && parts[2] == format!("index-{generation}.json.enc")
    {
        return source_id(parts[1]);
    }
    Err(integrity("transaction claims an unowned path"))
}
fn file_limit(path: &str) -> usize {
    if path.ends_with(".zip") {
        64 * 1024 * 1024
    } else {
        MAX_ENVELOPE_BYTES
    }
}
fn validate_archive_path(path: &str) -> Result<()> {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 4 || parts[0] != "sources" || parts[2] != "archives" {
        return Err(integrity("invalid managed archive path"));
    }
    source_id(parts[1])?;
    source_id(
        parts[3]
            .strip_suffix(".zip")
            .ok_or_else(|| integrity("invalid archive identity"))?,
    )
}
fn source_archives(source: &SourceIndex) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for artifact in source
        .current
        .values()
        .filter(|c| c.archive_retained)
        .map(|c| &c.artifact)
        .chain(source.history.values().map(|h| &h.artifact))
    {
        let path = format!("sources/{}/archives/{}.zip", source.id, artifact.id);
        validate_archive_path(&path)?;
        if result
            .insert(path, artifact.archive_digest.clone())
            .is_some_and(|old| old != artifact.archive_digest)
        {
            return Err(integrity("conflicting archive digests"));
        }
    }
    Ok(result)
}
fn validate_retired_path(path: &str, central: &CentralIndex, before: u64) -> Result<()> {
    if path == central_path(before) && before < central.generation {
        return Ok(());
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() == 3 && parts[0] == "sources" {
        source_id(parts[1])?;
        let generation: u64 = parts[2]
            .strip_prefix("index-")
            .and_then(|s| s.strip_suffix(".json.enc"))
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| integrity("invalid retired index path"))?;
        if generation > 0
            && generation <= before
            && central
                .sources
                .get(parts[1])
                .is_some_and(|selected| *selected > generation)
            && path == source_path(parts[1], generation)
        {
            return Ok(());
        }
    }
    Err(integrity(
        "transaction cannot retire a selected or unowned index",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mirror_fixture(
        parent: &Path,
        destination: &Path,
        unit: &UnitBinding,
        contents: &str,
        previous: Option<MirrorBinding>,
    ) -> (materialization::Prepared, MirrorChange, MirrorBinding) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
        zip.start_file(
            "file",
            zip::write::SimpleFileOptions::default().unix_permissions(0o644),
        )
        .unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let tree = crate::core::snapshots::tree_digest(
            &crate::core::snapshots::decode(&bytes, None).unwrap(),
        );
        let prepared = materialization::prepare(parent, &bytes, &tree, true).unwrap();
        let mut artifact = unit.artifact.content_record();
        artifact.id = digest(&bytes);
        artifact.tree_digest = tree.clone();
        artifact.archive_digest = digest(&bytes);
        artifact.archive_size = bytes.len() as u64;
        let directory = OwnedDirectory {
            path: destination.to_str().unwrap().into(),
            tree_digest: tree,
            generation: 1,
            entries: prepared.entries.clone(),
        };
        let mirror = MirrorBinding {
            artifact,
            directory,
        };
        let change = MirrorChange::new(
            destination.to_str().unwrap().into(),
            Some(prepared.staging.path().to_str().unwrap().into()),
            previous,
            Some(mirror.clone()),
        )
        .unwrap();
        (prepared, change, mirror)
    }
    #[test]
    fn mirror_replacement_and_removal_recover_at_every_cutover_step() {
        for remove in [false, true] {
            for fail_at in 0..=5 {
                let home = tempfile::tempdir().unwrap();
                let layout = Layout::under(home.path()).unwrap();
                layout.initialize_root().unwrap();
                let keys = Keys::derive(&[42; 32], "store").unwrap();
                let store = Generations {
                    layout: &layout,
                    keys: &keys,
                    store_id: "store",
                    key_generation: 1,
                };
                store
                    .initialize(&CentralIndex::empty("store".into()))
                    .unwrap();
                let (_staging, directory, unit) = prepare_directory(&store, 1, "central");
                let mut central = store.read().unwrap().central;
                central.units.insert(unit.id.clone(), unit.clone());
                store.commit_directory(0, central, directory).unwrap();
                let parent = std::fs::canonicalize(home.path()).unwrap();
                let destination = parent.join("mirror");
                let (_first_stage, first_change, first) =
                    mirror_fixture(&parent, &destination, &unit, "old", None);
                let mut central = store.read().unwrap().central;
                central
                    .units
                    .get_mut("unit")
                    .unwrap()
                    .mirrors
                    .push(first.clone());
                store.commit_mirror(1, central, first_change).unwrap();
                let mut central = store.read().unwrap().central;
                let (_staging, change, second) =
                    mirror_fixture(&parent, &destination, &unit, "new", Some(first));
                let mirrors = if remove {
                    central.units.get_mut("unit").unwrap().mirrors.clear();
                    vec![]
                } else {
                    central.units.get_mut("unit").unwrap().mirrors = vec![second];
                    vec![change]
                };
                assert!(
                    store
                        .publish_trees(
                            2,
                            central,
                            vec![],
                            vec![],
                            Trees {
                                directories: vec![],
                                mirrors
                            },
                            |step| {
                                if step == fail_at {
                                    Err(integrity("injected mirror interruption"))
                                } else {
                                    Ok(())
                                }
                            }
                        )
                        .is_err()
                );
                store.recover().unwrap();
                store.recover().unwrap();
                let selected = store.read().unwrap().central.units.remove("unit").unwrap();
                if fail_at < 4 {
                    assert_eq!(
                        std::fs::read_to_string(destination.join("file")).unwrap(),
                        "old"
                    );
                    assert_eq!(selected.mirrors.len(), 1);
                } else if remove {
                    assert!(!destination.exists());
                    assert!(selected.mirrors.is_empty());
                } else {
                    assert_eq!(
                        std::fs::read_to_string(destination.join("file")).unwrap(),
                        "new"
                    );
                    assert_eq!(selected.mirrors.len(), 1);
                }
                assert!(std::fs::read_dir(&parent).unwrap().all(|entry| {
                    !entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".saucepan-backup-")
                }));
            }
        }
    }
    fn prepare_directory(
        store: &Generations<'_>,
        generation: u64,
        contents: &str,
    ) -> (materialization::Prepared, DirectoryPublication, UnitBinding) {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
        zip.start_file(
            "file",
            zip::write::SimpleFileOptions::default().unix_permissions(0o644),
        )
        .unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let tree = crate::core::snapshots::tree_digest(
            &crate::core::snapshots::decode(&bytes, None).unwrap(),
        );
        let prepared = materialization::prepare(
            &store.layout.path(Path::new("transactions")).unwrap(),
            &bytes,
            &tree,
            true,
        )
        .unwrap();
        let owned = OwnedDirectory {
            path: format!("materializations/{generation:064x}"),
            tree_digest: tree.clone(),
            generation,
            entries: prepared.entries.clone(),
        };
        let publication = DirectoryPublication {
            staged: prepared
                .staging
                .path()
                .strip_prefix(store.layout.root())
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/"),
            owned: owned.clone(),
        };
        let unit = UnitBinding {
            id: "unit".into(),
            app_id: "app".into(),
            name: "tool".into(),
            recipe: Recipe::git("owner/repo"),
            artifact: ArtifactHandle {
                id: digest(&bytes),
                inputs: ResolvedInputs {
                    source_id: "a".repeat(64),
                    stream_id: "b".repeat(64),
                    subdirectory: ".".into(),
                    commit: "c".repeat(40),
                    dependencies: vec![],
                    lfs_objects: vec![],
                    export_version: 1,
                },
                archive_digest: digest(&bytes),
                tree_digest: tree,
                archive_size: bytes.len() as u64,
                disposition: ArtifactDisposition::Current,
                evidence: AcquisitionEvidence {
                    recipe_digest: "recipe".into(),
                    manifest: None,
                    policy_revision: 1,
                    content_rechecked: true,
                },
            },
            manifest: serde_json::json!({"name":"tool"}),
            materialization: Some(owned),
            mirrors: vec![],
        };
        (prepared, publication, unit)
    }
    #[test]
    fn directory_cutover_recovers_old_or_new_binding_at_every_publication_step() {
        for fail_at in 0..=4 {
            let home = tempfile::tempdir().unwrap();
            let layout = Layout::under(home.path()).unwrap();
            layout.initialize_root().unwrap();
            let keys = Keys::derive(&[42; 32], "store").unwrap();
            let store = Generations {
                layout: &layout,
                keys: &keys,
                store_id: "store",
                key_generation: 1,
            };
            store
                .initialize(&CentralIndex::empty("store".into()))
                .unwrap();
            let (_old_staging, first, unit) = prepare_directory(&store, 1, "old");
            let old_path = layout.path(Path::new(&first.owned.path)).unwrap();
            let mut central = store.read().unwrap().central;
            central.units.insert(unit.id.clone(), unit);
            store.commit_directory(0, central, first).unwrap();
            let (_staging, next, unit) = prepare_directory(&store, 2, "new");
            let new_path = layout.path(Path::new(&next.owned.path)).unwrap();
            let mut central = store.read().unwrap().central;
            central.units.insert(unit.id.clone(), unit);
            assert!(
                store
                    .publish(1, central, vec![], vec![], vec![next], |step| {
                        if step == fail_at {
                            Err(integrity("injected directory interruption"))
                        } else {
                            Ok(())
                        }
                    })
                    .is_err()
            );
            store.recover().unwrap();
            store.recover().unwrap();
            let selected = store
                .read()
                .unwrap()
                .central
                .units
                .remove("unit")
                .unwrap()
                .materialization
                .unwrap();
            if fail_at < 3 {
                assert_eq!(
                    std::fs::read_to_string(old_path.join("file")).unwrap(),
                    "old"
                );
                assert!(!new_path.exists());
                assert_eq!(selected.generation, 1);
            } else {
                assert_eq!(
                    std::fs::read_to_string(new_path.join("file")).unwrap(),
                    "new"
                );
                assert!(!old_path.exists());
                assert_eq!(selected.generation, 2);
            }
        }
    }
    #[test]
    fn interrupted_directory_cleanup_preserves_unrecognized_entries() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        let keys = Keys::derive(&[42; 32], "store").unwrap();
        let store = Generations {
            layout: &layout,
            keys: &keys,
            store_id: "store",
            key_generation: 1,
        };
        store
            .initialize(&CentralIndex::empty("store".into()))
            .unwrap();
        let (_staging, directory, unit) = prepare_directory(&store, 1, "new");
        let path = layout.path(Path::new(&directory.owned.path)).unwrap();
        let mut central = store.read().unwrap().central;
        central.units.insert(unit.id.clone(), unit);
        assert!(
            store
                .publish(0, central, vec![], vec![], vec![directory], |step| {
                    if step == 2 {
                        Err(integrity("injected interruption after rename"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        std::fs::write(path.join("unowned"), "keep").unwrap();
        assert!(store.recover().is_err());
        assert_eq!(std::fs::read_to_string(path.join("file")).unwrap(), "new");
        assert_eq!(
            std::fs::read_to_string(path.join("unowned")).unwrap(),
            "keep"
        );
        assert!(store.read().unwrap().central.units.is_empty());
        std::fs::remove_file(path.join("unowned")).unwrap();
        // Simulate a process dying after deleting one owned entry.
        std::fs::remove_file(path.join("file")).unwrap();
        store.recover().unwrap();
        assert!(!path.exists());
    }
    fn source(id: char) -> SourceIndex {
        SourceIndex {
            schema_version: 1,
            id: id.to_string().repeat(64),
            origin: format!("https://example.test/{id}.git"),
            generation: 0,
            sequence: 0,
            policy: CachePolicy::default(),
            current: BTreeMap::new(),
            history: BTreeMap::new(),
        }
    }
    #[test]
    fn rolling_archive_interruption_preserves_complete_old_or_new_history() {
        // Journal, source generation, central generation, new ZIP, commit, cleanup.
        for fail_at in 0..=5 {
            let home = tempfile::tempdir().unwrap();
            let layout = Layout::under(home.path()).unwrap();
            layout.initialize_root().unwrap();
            let keys = Keys::derive(&[42; 32], "store").unwrap();
            let store = Generations {
                layout: &layout,
                keys: &keys,
                store_id: "store",
                key_generation: 1,
            };
            store
                .initialize(&CentralIndex::empty("store".into()))
                .unwrap();
            let mut record = source('a');
            let stream = "b".repeat(64);
            let descriptor = StreamDescriptor {
                source_id: record.id.clone(),
                revision: Revision::DefaultBranch,
                export: Export::default(),
            };
            let artifact = |n: u8| ContentArtifact {
                id: digest(&[n]),
                inputs: ResolvedInputs {
                    source_id: record.id.clone(),
                    stream_id: stream.clone(),
                    subdirectory: ".".into(),
                    commit: format!("{n:040x}"),
                    dependencies: vec![],
                    lfs_objects: vec![],
                    export_version: 1,
                },
                archive_digest: digest(&[n]),
                tree_digest: digest(&[n]),
                archive_size: 1,
            };
            let artifacts: Vec<_> = (0..7).map(artifact).collect();
            let archive_path = |a: &ContentArtifact| {
                format!("sources/{}/archives/{}.zip", a.inputs.source_id, a.id)
            };
            layout
                .create_directory(Path::new(&format!("sources/{}", record.id)))
                .unwrap();
            layout
                .create_directory(Path::new(&format!("sources/{}/archives", record.id)))
                .unwrap();
            for a in &artifacts[..6] {
                crate::core::snapshots::advance(
                    &mut record,
                    &stream,
                    descriptor.clone(),
                    a.clone(),
                    true,
                )
                .unwrap();
            }
            store
                .commit_archives(
                    0,
                    store.read().unwrap().central,
                    vec![record],
                    artifacts[..6]
                        .iter()
                        .enumerate()
                        .map(|(n, a)| (archive_path(a), vec![n as u8]))
                        .collect(),
                )
                .unwrap();
            let old = store.read().unwrap();
            let mut next = old.sources[&"a".repeat(64)].clone();
            crate::core::snapshots::advance(
                &mut next,
                &stream,
                descriptor,
                artifacts[6].clone(),
                true,
            )
            .unwrap();
            assert!(
                store
                    .publish(
                        1,
                        old.central,
                        vec![next],
                        vec![(archive_path(&artifacts[6]), vec![6])],
                        vec![],
                        |step| {
                            if step == fail_at {
                                Err(integrity("injected archive interruption"))
                            } else {
                                Ok(())
                            }
                        }
                    )
                    .is_err()
            );
            store.recover().unwrap();
            store.recover().unwrap();
            let committed = fail_at >= 4;
            let snapshot = store.read().unwrap();
            let selected = &snapshot.sources[&"a".repeat(64)];
            assert_eq!(selected.history.len(), 5);
            assert_eq!(
                selected.current[&stream].artifact.id,
                artifacts[if committed { 6 } else { 5 }].id
            );
            let expected = if committed { 1..7 } else { 0..6 };
            for (n, a) in artifacts.iter().enumerate() {
                assert_eq!(
                    layout.path(Path::new(&archive_path(a))).unwrap().exists(),
                    expected.contains(&n),
                    "fault {fail_at}, archive {n}"
                );
            }
        }
    }
    #[test]
    fn every_publication_interruption_selects_one_complete_generation() {
        // journal, source A, source B, immutable central, commit, cleanup
        for fail_at in 0..=5 {
            let home = tempfile::tempdir().unwrap();
            let layout = Layout::under(home.path()).unwrap();
            layout.initialize_root().unwrap();
            let keys = Keys::derive(&[42; 32], "store").unwrap();
            let store = Generations {
                layout: &layout,
                keys: &keys,
                store_id: "store",
                key_generation: 1,
            };
            store
                .initialize(&CentralIndex::empty("store".into()))
                .unwrap();
            let mut next = store.read().unwrap().central;
            next.catalogs.insert("app".into(), vec![]);
            let result = store.commit_steps(0, next, vec![source('a'), source('b')], |step| {
                if step == fail_at {
                    Err(Error::new(
                        ErrorKind::Internal,
                        "injected process interruption",
                    ))
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err());
            store.recover().unwrap();
            store.recover().unwrap();
            let snapshot = store.read().unwrap();
            let committed = fail_at >= 4;
            assert_eq!(snapshot.central.generation, u64::from(committed));
            assert_eq!(snapshot.sources.len(), if committed { 2 } else { 0 });
            assert_eq!(snapshot.central.catalogs.contains_key("app"), committed);
            assert!(!layout.root().join(JOURNAL).exists());
            if !committed {
                store
                    .commit(0, snapshot.central, vec![source('a'), source('b')])
                    .unwrap();
            }
        }
    }
    #[test]
    fn stale_generation_and_cross_source_substitution_fail_closed() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        let keys = Keys::derive(&[42; 32], "store").unwrap();
        let store = Generations {
            layout: &layout,
            keys: &keys,
            store_id: "store",
            key_generation: 1,
        };
        store
            .initialize(&CentralIndex::empty("store".into()))
            .unwrap();
        let stale = store.read().unwrap().central;
        store
            .commit(0, stale.clone(), vec![source('a'), source('b')])
            .unwrap();
        assert_eq!(
            store.commit(0, stale, vec![]).unwrap_err().kind,
            ErrorKind::Busy
        );
        let bytes = layout
            .read(
                Path::new(&source_path(&"a".repeat(64), 1)),
                MAX_ENVELOPE_BYTES,
            )
            .unwrap();
        layout
            .replace(Path::new(&source_path(&"b".repeat(64), 1)), &bytes)
            .unwrap();
        assert!(matches!(store.read(), Err(e) if e.kind == ErrorKind::Integrity));
    }
    #[test]
    fn recovery_preserves_modified_staging_and_unowned_files() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        let keys = Keys::derive(&[42; 32], "store").unwrap();
        let store = Generations {
            layout: &layout,
            keys: &keys,
            store_id: "store",
            key_generation: 1,
        };
        store
            .initialize(&CentralIndex::empty("store".into()))
            .unwrap();
        let old = store.read().unwrap().central;
        assert!(
            store
                .commit_steps(0, old, vec![source('a')], |s| {
                    if s == 1 {
                        Err(integrity("interrupted"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
        );
        let path = source_path(&"a".repeat(64), 1);
        layout.replace(Path::new(&path), b"user-edited").unwrap();
        layout.create_file(Path::new("keep"), b"original").unwrap();
        assert_eq!(store.recover().unwrap_err().kind, ErrorKind::Integrity);
        assert_eq!(layout.read(Path::new(&path), 100).unwrap(), b"user-edited");
        assert_eq!(layout.read(Path::new("keep"), 100).unwrap(), b"original");
        assert_eq!(store.read().unwrap().central.generation, 0);
    }
    #[test]
    fn interruption_during_abort_or_retirement_cleanup_is_idempotent() {
        for committed in [false, true] {
            for fail_at in 0..2 {
                let home = tempfile::tempdir().unwrap();
                let layout = Layout::under(home.path()).unwrap();
                layout.initialize_root().unwrap();
                let keys = Keys::derive(&[42; 32], "store").unwrap();
                let store = Generations {
                    layout: &layout,
                    keys: &keys,
                    store_id: "store",
                    key_generation: 1,
                };
                store
                    .initialize(&CentralIndex::empty("store".into()))
                    .unwrap();
                store
                    .commit(0, store.read().unwrap().central, vec![source('a')])
                    .unwrap();
                let snapshot = store.read().unwrap();
                let mut changed = snapshot.sources[&"a".repeat(64)].clone();
                changed.sequence = 10;
                let stop = if committed { 3 } else { 2 };
                assert!(
                    store
                        .commit_steps(1, snapshot.central, vec![changed], |step| {
                            if step == stop {
                                Err(integrity("interrupted"))
                            } else {
                                Ok(())
                            }
                        })
                        .is_err()
                );
                assert!(
                    store
                        .recover_steps(|step| {
                            if step == fail_at {
                                Err(integrity("cleanup interrupted"))
                            } else {
                                Ok(())
                            }
                        })
                        .is_err()
                );
                store.recover().unwrap();
                store.recover().unwrap();
                let snapshot = store.read().unwrap();
                assert_eq!(snapshot.central.generation, if committed { 2 } else { 1 });
                assert_eq!(
                    snapshot.sources[&"a".repeat(64)].sequence,
                    if committed { 10 } else { 0 }
                );
                assert!(
                    !layout
                        .root()
                        .join(central_path(if committed { 1 } else { 2 }))
                        .exists()
                );
                assert!(
                    !layout
                        .root()
                        .join(source_path(&"a".repeat(64), if committed { 1 } else { 2 }))
                        .exists()
                );
            }
        }
    }
}
