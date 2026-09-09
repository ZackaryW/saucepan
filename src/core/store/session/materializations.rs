use super::super::transactions::DirectoryPublication;
use super::*;
use crate::core::{authority, materialization, recipes, sources};

impl Session {
    pub(in crate::core) fn materialize(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        recipe: &Recipe,
        artifact_id: &str,
        verification: &VerificationRequest,
    ) -> Result<MaterializedArtifact> {
        let context = self.inspect_context(root, marker)?;
        recipes::validate(recipe)?;
        let identity = sources::identify(&recipe.source, Path::new(&context.root))?;
        authority::require_source(
            &context,
            &identity.id,
            &recipe.export.subdirectory,
            Action::Setup,
        )?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        if !self.read_index()?.sources.contains_key(&identity.id) {
            return Err(Error::not_found());
        }
        let mut access = self.access_source(
            root,
            marker,
            &identity,
            &recipe.export.subdirectory,
            Action::Setup,
            verification,
        )?;
        let (record, retained, disposition) = access
            .source
            .history
            .get(artifact_id)
            .map(|history| (&history.artifact, true, ArtifactDisposition::Historical))
            .or_else(|| {
                access
                    .source
                    .current
                    .values()
                    .find(|current| current.artifact.id == artifact_id)
                    .map(|current| {
                        (
                            &current.artifact,
                            current.archive_retained,
                            if current.archive_retained {
                                ArtifactDisposition::Current
                            } else {
                                ArtifactDisposition::Temporary
                            },
                        )
                    })
            })
            .filter(|(artifact, _, _)| {
                artifact.inputs.subdirectory == recipe.export.subdirectory
                    && artifact.inputs.export_version == recipe.export.version
            })
            .ok_or_else(Error::not_found)?;
        let record = record.clone();
        self.guard_dependencies(&mut access, &record.inputs)?;
        let repository = sources::GitRepository::open(&access.repository, &identity)?;
        let bytes = if retained {
            self.read_archive(&access, &record, access.policy.verify_content)?
        } else {
            let exported = self.reconstruct(&mut access, &repository, &record)?;
            if exported.tree_digest != record.tree_digest
                || exported.lfs_objects != record.inputs.lfs_objects
            {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "reconstructed artifact tree changed",
                ));
            }
            exported.bytes
        };
        let prepared = materialization::prepare(
            &self.layout.path(Path::new("transactions"))?,
            &bytes,
            &record.tree_digest,
            access.policy.verify_content,
        )?;
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce)
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        let id = utils::hex(&nonce);
        let relative = format!("materializations/{id}");
        let staged = prepared
            .staging
            .path()
            .strip_prefix(self.layout.root())
            .map_err(|_| {
                Error::new(
                    ErrorKind::Integrity,
                    "materialization staging escaped store",
                )
            })?
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "invalid staging path"))?
            .replace('\\', "/");
        let recipe_bytes = serde_json::to_vec(recipe)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode recipe evidence"))?;
        let artifact = ArtifactHandle {
            id: record.id.clone(),
            inputs: record.inputs.clone(),
            archive_digest: record.archive_digest.clone(),
            tree_digest: record.tree_digest.clone(),
            archive_size: record.archive_size,
            disposition,
            evidence: AcquisitionEvidence {
                recipe_digest: utils::hex(&Sha256::digest(&recipe_bytes)),
                manifest: recipe.manifest.clone(),
                policy_revision: access.policy.revision,
                content_rechecked: access.policy.verify_content,
            },
        };
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let mut snapshot = self.generations().read()?;
        self.revalidate_source(&access, &snapshot)?;
        if context.revision != access.context.revision {
            return Err(Error::new(
                ErrorKind::Authority,
                "application changed during materialization",
            ));
        }
        repository.verify_origin()?;
        let app = snapshot
            .central
            .applications
            .get_mut(&context.id)
            .ok_or_else(Error::not_found)?;
        app.data_generation = app
            .data_generation
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "application generation exhausted"))?;
        let owned = OwnedDirectory {
            path: relative.clone(),
            tree_digest: artifact.tree_digest.clone(),
            generation: app.data_generation,
            entries: prepared.entries,
        };
        let binding = MaterializationBinding {
            id: id.clone(),
            app_id: context.id.clone(),
            recipe: recipe.clone(),
            artifact: artifact.clone(),
            directory: owned.clone(),
            mirrors: vec![],
        };
        snapshot
            .central
            .materializations
            .insert(id.clone(), binding);
        let mut source = access.source.clone();
        source.sequence = source
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "source recency exhausted"))?;
        if let Some(history) = source.history.get_mut(artifact_id) {
            history.last_used = source.sequence;
        }
        for current in source
            .current
            .values_mut()
            .filter(|current| current.artifact.id == artifact_id)
        {
            current.last_used = source.sequence;
        }
        self.generations().commit_materialization(
            snapshot.central.generation,
            snapshot.central,
            source,
            DirectoryPublication { staged, owned },
        )?;
        self.reconcile_pins(&access)?;
        let path = self
            .layout
            .path(Path::new(&relative))?
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "invalid materialization path"))?
            .into();
        Ok(MaterializedArtifact { id, path, artifact })
    }

    pub(in crate::core) fn materialization_path(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        id: &str,
        verification: &VerificationRequest,
    ) -> Result<std::path::PathBuf> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let binding = self
            .read_index()?
            .materializations
            .remove(id)
            .filter(|binding| binding.app_id == context.id)
            .ok_or_else(Error::not_found)?;
        authority::require_source(
            &context,
            &binding.artifact.inputs.source_id,
            &binding.artifact.inputs.subdirectory,
            Action::Inspect,
        )?;
        let identity = sources::identify(&binding.recipe.source, Path::new(&context.root))?;
        let mut access = self.inspect_source(
            root,
            marker,
            &identity,
            &binding.artifact.inputs.subdirectory,
            verification,
        )?;
        self.guard_dependencies(&mut access, &binding.artifact.inputs)?;
        let repository = sources::GitRepository::open(&access.repository, &identity)?;
        let path = self.layout.path(Path::new(&binding.directory.path))?;
        if !path.is_dir() {
            return Err(Error::not_found());
        }
        if access.policy.verify_content
            && materialization::check(&path, &binding.directory.entries, false)?
                != binding.artifact.tree_digest
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "materialized tree does not match authenticated artifact",
            ));
        }
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        self.revalidate_source(&access, &self.generations().read()?)?;
        if context.revision != access.context.revision {
            return Err(Error::new(
                ErrorKind::Authority,
                "application changed during materialization lookup",
            ));
        }
        repository.verify_origin()?;
        Ok(path)
    }

    pub(in crate::core) fn release_materialization(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        id: &str,
    ) -> Result<()> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let binding = self
            .read_index()?
            .materializations
            .remove(id)
            .filter(|binding| binding.app_id == context.id)
            .ok_or_else(Error::not_found)?;
        authority::require_source(
            &context,
            &binding.artifact.inputs.source_id,
            &binding.artifact.inputs.subdirectory,
            Action::Remove,
        )
        .map_err(|_| Error::not_found())?;
        let identity = sources::identify(&binding.recipe.source, Path::new(&context.root))?;
        let mut access = self.access_source(
            root,
            marker,
            &identity,
            &binding.artifact.inputs.subdirectory,
            Action::Remove,
            &VerificationRequest { content: None },
        )?;
        self.guard_dependencies(&mut access, &binding.artifact.inputs)?;
        for mirror in &binding.mirrors {
            self.guard_dependencies(&mut access, &mirror.artifact.inputs)?;
        }
        let repository = sources::GitRepository::open(&access.repository, &identity)?;
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let mut snapshot = self.generations().read()?;
        self.revalidate_source(&access, &snapshot)?;
        if context.revision != access.context.revision {
            return Err(Error::new(
                ErrorKind::Authority,
                "application changed during materialization removal",
            ));
        }
        repository.verify_origin()?;
        for mirror in &binding.mirrors {
            authority::require_artifact_destination(
                &access.context,
                &mirror.artifact.inputs,
                Action::Remove,
                Path::new(&mirror.directory.path),
            )?;
        }
        snapshot
            .central
            .materializations
            .remove(id)
            .ok_or_else(Error::not_found)?;
        let app = snapshot
            .central
            .applications
            .get_mut(&context.id)
            .ok_or_else(Error::not_found)?;
        app.data_generation = app
            .data_generation
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "application generation exhausted"))?;
        self.generations()
            .commit(snapshot.central.generation, snapshot.central, vec![])?;
        self.reconcile_pins(&access)
    }
}
