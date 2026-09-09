use super::{Store, models::*};
use super::{content, sources};
use crate::utils::{archive, fs::atomic_write, hash::sha256, tree};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};

impl Store {
    pub fn acquire(&self, context: &AppContext, recipe: &Recipe) -> Result<Acquired> {
        let _lock = self.lock()?;
        let mut index = self.read()?;
        let settings = self.access(&index, context)?.settings.clone();
        recipe.validate()?;
        let source = recipe.source.canonical()?;
        let id = source.id()?;
        let source_root = self.root.join("sources").join(&id);
        fs::create_dir_all(&source_root)?;
        let prepared = match sources::prepare(&source, &source_root, recipe.commit.as_deref()) {
            Ok(prepared) => prepared,
            Err(error)
                if error.is::<sources::RemoteUnavailable>() && settings.allow_local_fallback =>
            {
                return self.fallback(&mut index, context, &id, recipe, settings.verify_content);
            }
            Err(error) => return Err(error),
        };
        let input = prepared.directory.path().join("tree");
        let mut files = content::files(&input)?;
        if let Some(executables) = &prepared.executables {
            for (path, record) in &mut files {
                record.executable = executables.contains(path);
            }
        }
        let content_id = content::digest(&files)?;
        let snapshot_id =
            content::digest(&(&id, &prepared.revision, &content_id, &prepared.dependencies))?;
        let state = index
            .sources
            .entry(id.clone())
            .or_insert_with(|| SourceState {
                source,
                current: None,
                history: Vec::new(),
                sequence: 0,
            });
        state.sequence = state
            .sequence
            .checked_add(1)
            .context("source sequence exhausted")?;
        let mut snapshot = Snapshot {
            id: snapshot_id,
            revision: prepared.revision,
            content_id,
            files,
            dependencies: prepared.dependencies,
            zip_digest: None,
            last_used: state.sequence,
            created: state.sequence,
        };
        if settings.retain_snapshots
            && recipe.commit.is_none()
            && let Some(outgoing) = state
                .current
                .as_ref()
                .filter(|s| s.id != snapshot.id && s.zip_digest.is_some())
        {
            let zip = source_root
                .join("snapshots")
                .join(format!("{}.zip", outgoing.id));
            ensure!(zip.is_file(), "outgoing retained snapshot is missing");
            ensure!(
                Some(sha256(fs::File::open(zip)?)?) == outgoing.zip_digest,
                "outgoing retained snapshot is corrupt"
            );
        }
        let artifact = artifact(&id, &state.source, &snapshot, recipe.folder.clone(), &input)?;
        let destination = source_root.join("content").join(&snapshot.id);
        fs::create_dir_all(source_root.join("content"))?;
        if destination.try_exists()? {
            content::verify(
                &content::folder(&destination, recipe.folder.as_deref())?,
                &artifact.files,
                settings.verify_content,
            )?;
        } else {
            tree::copy_tree(&input, &destination, |_, _| true)?;
        }
        if let Some(previous) = state
            .current
            .iter()
            .chain(state.history.iter())
            .find(|previous| previous.id == snapshot.id)
        {
            snapshot.created = previous.created;
            snapshot.zip_digest = previous.zip_digest.clone();
        }
        if settings.retain_snapshots && snapshot.zip_digest.is_none() {
            fs::create_dir_all(source_root.join("snapshots"))?;
            let zip = source_root
                .join("snapshots")
                .join(format!("{}.zip", snapshot.id));
            if !zip.try_exists()? {
                atomic_write(&zip, |writer| {
                    archive::write_zip_with(
                        &input,
                        writer,
                        |_, _| true,
                        |entry, options| {
                            options.unix_permissions(
                                if entry.directory || snapshot.files[&entry.path].executable {
                                    0o755
                                } else {
                                    0o644
                                },
                            )
                        },
                    )?;
                    Ok(())
                })?;
            } else {
                let temporary = tempfile::tempdir_in(&source_root)?;
                let check = temporary.path().join("check");
                archive::extract_zip(fs::File::open(&zip)?, &check, None, sources::ZIP_LIMITS)?;
                content::verify(&check, &snapshot.files, true)?;
            }
            snapshot.zip_digest = Some(sha256(fs::File::open(zip)?)?);
        }
        let mut evicted = Vec::new();
        if recipe.commit.is_none() {
            evicted = super::policies::advance(state, snapshot.clone(), settings.retain_snapshots);
        } else if let Some(previous) = state
            .current
            .iter_mut()
            .chain(state.history.iter_mut())
            .find(|s| s.id == snapshot.id)
        {
            *previous = snapshot.clone();
        } else if settings.retain_snapshots {
            state.history.push(snapshot.clone());
            evicted = super::policies::trim_history(state);
        }
        let result = Acquired {
            directory: content::folder(&destination, recipe.folder.as_deref())?,
            artifact: artifact.clone(),
            fallback: false,
            update_checked: recipe.commit.is_none(),
            content_verified: settings.verify_content,
        };
        index
            .artifacts
            .insert(artifact.id.clone(), artifact.clone());
        index
            .apps
            .get_mut(&context.app)
            .context("app disappeared")?
            .touched
            .insert(artifact.id);
        self.save(&mut index)?;
        // Garbage collection cannot invalidate a successful publication or live content.
        for snapshot in evicted {
            let _ = fs::remove_file(
                source_root
                    .join("snapshots")
                    .join(format!("{snapshot}.zip")),
            );
        }
        Ok(result)
    }
    pub fn source_state(&self, context: &AppContext, id: &str) -> Result<Option<SourceState>> {
        let _lock = self.lock()?;
        let index = self.read()?;
        self.access(&index, context)?;
        if !Self::select(&index, &context.app)?
            .entries
            .values()
            .any(|a| a.source_id == id)
        {
            return Ok(None);
        }
        Ok(index.sources.get(id).cloned())
    }
    pub fn artifact_path(&self, context: &AppContext, id: &str) -> Result<Option<PathBuf>> {
        let _lock = self.lock()?;
        let index = self.read()?;
        let app = self.access(&index, context)?;
        let view = Self::select(&index, &context.app)?;
        let Some(artifact) = view.entries.get(id) else {
            return Ok(None);
        };
        let root = self
            .root
            .join("sources")
            .join(&artifact.source_id)
            .join("content")
            .join(&artifact.snapshot_id);
        let path = content::folder(&root, artifact.folder.as_deref())?;
        content::verify(&path, &artifact.files, app.settings.verify_content)?;
        Ok(Some(path))
    }
    pub fn mirror(
        &self,
        context: &AppContext,
        id: &str,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let path = self
            .artifact_path(context, id)?
            .context("artifact is not in the app view")?;
        tree::copy_tree(path, destination, |_, _| true)?;
        Ok(())
    }
    pub fn read_snapshot(
        &self,
        context: &AppContext,
        source_id: &str,
        snapshot_id: &str,
        folder: Option<String>,
    ) -> Result<Acquired> {
        let _lock = self.lock()?;
        let mut index = self.read()?;
        let settings = self.access(&index, context)?.settings.clone();
        ensure!(
            Self::select(&index, &context.app)?
                .entries
                .values()
                .any(|a| a.source_id == source_id),
            "source is not in the app view"
        );
        let state = index
            .sources
            .get_mut(source_id)
            .context("source is missing")?;
        let snapshot = state
            .current
            .iter_mut()
            .chain(state.history.iter_mut())
            .find(|s| s.id == snapshot_id)
            .context("snapshot is not retained")?;
        let zip = self
            .root
            .join("sources")
            .join(source_id)
            .join("snapshots")
            .join(format!("{snapshot_id}.zip"));
        let expected = snapshot
            .zip_digest
            .as_ref()
            .context("snapshot has no retained ZIP")?;
        ensure!(zip.is_file(), "retained ZIP is missing");
        if settings.verify_content {
            ensure!(
                &sha256(fs::File::open(&zip)?)? == expected,
                "ZIP digest mismatch"
            );
        }
        let temp = tempfile::tempdir_in(&self.root)?;
        let extracted = temp.path().join("tree");
        archive::extract_zip(
            fs::File::open(&zip)?,
            &extracted,
            None,
            archive::Limits {
                entries: 1_000_000,
                bytes: 16 * 1024 * 1024 * 1024,
            },
        )?;
        content::verify(&extracted, &snapshot.files, settings.verify_content)?;
        let artifact = artifact(
            source_id,
            &state.source,
            snapshot,
            folder.clone(),
            &extracted,
        )?;
        let central = self
            .root
            .join("sources")
            .join(source_id)
            .join("content")
            .join(snapshot_id);
        if !central.try_exists()? {
            tree::copy_tree(&extracted, &central, |_, _| true)?;
        }
        let path = content::folder(&central, folder.as_deref())?;
        content::verify(&path, &artifact.files, settings.verify_content)?;
        state.sequence = state
            .sequence
            .checked_add(1)
            .context("source sequence exhausted")?;
        snapshot.last_used = state.sequence;
        index
            .artifacts
            .insert(artifact.id.clone(), artifact.clone());
        index
            .apps
            .get_mut(&context.app)
            .context("app disappeared")?
            .touched
            .insert(artifact.id.clone());
        self.save(&mut index)?;
        Ok(Acquired {
            artifact,
            directory: path,
            fallback: false,
            update_checked: false,
            content_verified: settings.verify_content,
        })
    }

    fn fallback(
        &self,
        index: &mut Index,
        context: &AppContext,
        id: &str,
        recipe: &Recipe,
        checked: bool,
    ) -> Result<Acquired> {
        let state = index
            .sources
            .get_mut(id)
            .context("no recorded local copy for fallback")?;
        let snapshot = if let Some(commit) = &recipe.commit {
            state
                .current
                .iter_mut()
                .chain(state.history.iter_mut())
                .find(|s| s.revision == *commit)
                .context("no matching pinned local copy")?
        } else {
            state.current.as_mut().context("no current local copy")?
        };
        let root = self
            .root
            .join("sources")
            .join(id)
            .join("content")
            .join(&snapshot.id);
        let directory = content::folder(&root, recipe.folder.as_deref())?;
        let artifact = artifact(id, &state.source, snapshot, recipe.folder.clone(), &root)?;
        content::verify(&directory, &artifact.files, checked)?;
        state.sequence = state
            .sequence
            .checked_add(1)
            .context("source sequence exhausted")?;
        snapshot.last_used = state.sequence;
        index
            .artifacts
            .insert(artifact.id.clone(), artifact.clone());
        index
            .apps
            .get_mut(&context.app)
            .context("app disappeared")?
            .touched
            .insert(artifact.id.clone());
        self.save(index)?;
        Ok(Acquired {
            artifact,
            directory,
            fallback: true,
            update_checked: false,
            content_verified: checked,
        })
    }
}

fn artifact(
    source_id: &str,
    source: &Source,
    snapshot: &Snapshot,
    folder: Option<String>,
    root: &Path,
) -> Result<Artifact> {
    content::folder(root, folder.as_deref())?;
    let prefix = folder.as_ref().map(|folder| format!("{folder}/"));
    let files = snapshot
        .files
        .iter()
        .filter_map(|(path, record)| {
            let selected = match &prefix {
                Some(prefix) => path.strip_prefix(prefix)?,
                None => path.as_str(),
            };
            Some((selected.to_owned(), record.clone()))
        })
        .collect();
    Ok(Artifact {
        id: content::digest(&(source_id, &snapshot.id, &folder))?,
        source_id: source_id.into(),
        source: source.clone(),
        snapshot_id: snapshot.id.clone(),
        revision: snapshot.revision.clone(),
        folder,
        content_id: content::digest(&files)?,
        files,
    })
}

#[cfg(test)]
mod git_tests;
#[cfg(test)]
mod tests;
