use super::super::transactions::DirectoryPublication;
use super::*;
use crate::core::{acquisition, authority, materialization, recipes, sources};

enum Request<'a> {
    Install(&'a Recipe),
    Update(&'a str),
}

impl Session {
    pub(in crate::core) fn install(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        recipe: &Recipe,
        verification: &VerificationRequest,
    ) -> Result<UnitBinding> {
        self.change_installation(root, marker, Request::Install(recipe), verification)
    }
    pub(in crate::core) fn update(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        name: &str,
        verification: &VerificationRequest,
    ) -> Result<UnitBinding> {
        self.change_installation(root, marker, Request::Update(name), verification)
    }
    fn change_installation(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        request: Request<'_>,
        verification: &VerificationRequest,
    ) -> Result<UnitBinding> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let (recipe, action, expected_name) = match request {
            Request::Install(recipe) => (recipe.clone(), Action::Setup, None),
            Request::Update(name) => {
                let unit = self
                    .read_index()?
                    .units
                    .remove(&binding_id(&context.id, name))
                    .ok_or_else(Error::not_found)?;
                authority::require_source(
                    &context,
                    &unit.artifact.inputs.source_id,
                    &unit.artifact.inputs.subdirectory,
                    Action::Update,
                )
                .map_err(|_| Error::not_found())?;
                (unit.recipe, Action::Update, Some(name))
            }
        };
        let recipe = &recipe;
        recipes::validate(recipe)?;
        let mut artifact =
            acquisition::acquire_for_action(self, root, marker, recipe, verification, action)?;
        let identity = sources::identify(&recipe.source, Path::new(&context.root))?;
        let mut access = self.access_source(
            root,
            marker,
            &identity,
            &recipe.export.subdirectory,
            action,
            verification,
        )?;
        let (record, retained) = access
            .source
            .current
            .values()
            .find(|c| c.artifact.id == artifact.id)
            .map(|c| (&c.artifact, c.archive_retained))
            .or_else(|| {
                access
                    .source
                    .history
                    .get(&artifact.id)
                    .map(|h| (&h.artifact, true))
            })
            .ok_or_else(Error::not_found)?;
        if !record.same_content(&artifact.content_record()) {
            return Err(Error::new(
                ErrorKind::Integrity,
                "artifact changed before installation",
            ));
        }
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
                    "reconstructed install tree changed",
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
        let manifest = if let Some(manifest) = &recipe.manifest {
            manifest.clone()
        } else {
            // Only a regular committed root manifest is eligible. Never follow
            // a symlink or infer a manifest from another application's binding.
            let entry = prepared
                .entries
                .get("sauce.json")
                .ok_or_else(Error::not_found)?;
            if entry.mode & 0o170000 != 0o100000 || entry.size > 1024 * 1024 {
                return Err(Error::new(ErrorKind::Config, "invalid repository manifest"));
            }
            let bytes = std::fs::read(prepared.staging.path().join("sauce.json"))
                .map_err(|_| Error::new(ErrorKind::Config, "cannot read repository manifest"))?;
            ManifestInput {
                document: serde_json::from_slice(&bytes).map_err(|_| {
                    Error::new(ErrorKind::Config, "invalid repository manifest JSON")
                })?,
                provenance: ManifestProvenance::Repository {
                    path: "sauce.json".into(),
                },
            }
        };
        let name = manifest_name(&manifest.document)?.to_owned();
        if expected_name.is_some_and(|expected| expected != name) {
            return Err(Error::new(
                ErrorKind::Conflict,
                "update changed the installed manifest name",
            ));
        }
        let id = binding_id(&context.id, &name);
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce)
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        let relative = format!("materializations/{}", utils::hex(&nonce));
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
                "application changed during installation",
            ));
        }
        repository.verify_origin()?;
        if let Some(old) = snapshot.central.units.get(&id) {
            if old.artifact.inputs.source_id != artifact.inputs.source_id {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "installed name belongs to another source origin",
                ));
            }
            authority::require_source(
                &access.context,
                &identity.id,
                &old.artifact.inputs.subdirectory,
                action,
            )?;
        }
        let next = snapshot
            .central
            .applications
            .get(&context.id)
            .ok_or_else(Error::not_found)?
            .data_generation
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "binding generation exhausted"))?;
        artifact.evidence.manifest = Some(manifest.clone());
        artifact.evidence.policy_revision = access.policy.revision;
        artifact.evidence.content_rechecked = access.policy.verify_content;
        let owned = OwnedDirectory {
            path: relative,
            tree_digest: artifact.tree_digest.clone(),
            generation: next,
            entries: prepared.entries,
        };
        let unit = UnitBinding {
            id: id.clone(),
            app_id: context.id.clone(),
            name,
            recipe: recipe.clone(),
            artifact,
            manifest: manifest.document,
            materialization: Some(owned.clone()),
            mirrors: snapshot
                .central
                .units
                .get(&id)
                .map(|u| u.mirrors.clone())
                .unwrap_or_default(),
        };
        snapshot.central.units.insert(id, unit.clone());
        let app = snapshot
            .central
            .applications
            .get_mut(&context.id)
            .ok_or_else(Error::not_found)?;
        app.data_generation = app
            .data_generation
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "application generation exhausted"))?;
        self.generations().commit_directory(
            snapshot.central.generation,
            snapshot.central,
            DirectoryPublication { staged, owned },
        )?;
        self.reconcile_pins(&access)?;
        Ok(unit)
    }

    pub(in crate::core) fn uninstall(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        name: &str,
    ) -> Result<()> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let unit = self
            .read_index()?
            .units
            .remove(&binding_id(&context.id, name))
            .ok_or_else(Error::not_found)?;
        authority::require_source(
            &context,
            &unit.artifact.inputs.source_id,
            &unit.artifact.inputs.subdirectory,
            Action::Remove,
        )
        .map_err(|_| Error::not_found())?;
        let identity = sources::identify(&unit.recipe.source, Path::new(&context.root))?;
        let mut access = self.access_source(
            root,
            marker,
            &identity,
            &unit.artifact.inputs.subdirectory,
            Action::Remove,
            &VerificationRequest { content: None },
        )?;
        self.guard_dependencies(&mut access, &unit.artifact.inputs)?;
        for mirror in &unit.mirrors {
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
                "application changed during removal",
            ));
        }
        repository.verify_origin()?;
        for mirror in &unit.mirrors {
            authority::require_artifact_destination(
                &access.context,
                &mirror.artifact.inputs,
                Action::Remove,
                Path::new(&mirror.directory.path),
            )?;
        }
        snapshot
            .central
            .units
            .remove(&unit.id)
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

    pub(in crate::core) fn unit_path(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        name: &str,
        verification: &VerificationRequest,
    ) -> Result<std::path::PathBuf> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let unit = self
            .read_index()?
            .units
            .remove(&binding_id(&context.id, name))
            .ok_or_else(Error::not_found)?;
        authority::require_source(
            &context,
            &unit.artifact.inputs.source_id,
            &unit.artifact.inputs.subdirectory,
            Action::Inspect,
        )?;
        let identity = sources::identify(&unit.recipe.source, Path::new(&context.root))?;
        let mut access = self.inspect_source(
            root,
            marker,
            &identity,
            &unit.artifact.inputs.subdirectory,
            verification,
        )?;
        self.guard_dependencies(&mut access, &unit.artifact.inputs)?;
        let repository = sources::GitRepository::open(&access.repository, &identity)?;
        let owned = unit.materialization.as_ref().ok_or_else(Error::not_found)?;
        let path = self.layout.path(Path::new(&owned.path))?;
        if !path.is_dir() {
            return Err(Error::not_found());
        }
        if access.policy.verify_content
            && materialization::check(&path, &owned.entries, false)? != unit.artifact.tree_digest
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
        let snapshot = self.generations().read()?;
        self.revalidate_source(&access, &snapshot)?;
        if context.revision != access.context.revision {
            return Err(Error::new(
                ErrorKind::Authority,
                "application changed during path lookup",
            ));
        }
        repository.verify_origin()?;
        Ok(path)
    }
}
pub(super) fn binding_id(app: &str, name: &str) -> String {
    utils::hex(&Sha256::digest(utils::frame(
        b"saucepan/unit/v1",
        &[app.as_bytes(), name.as_bytes()],
    )))
}
fn manifest_name(document: &serde_json::Value) -> Result<&str> {
    for field in ["name", "version", "description"] {
        if document.get(field).and_then(|v| v.as_str()).is_none() {
            return Err(Error::new(
                ErrorKind::Config,
                "manifest requires string name, version and description",
            ));
        }
    }
    let name = document["name"].as_str().expect("validated name");
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(Error::new(
            ErrorKind::Config,
            "invalid installed manifest name",
        ));
    }
    Ok(name)
}
