use super::super::{layout::external_path, mirrors::MirrorChange};
use super::*;
use crate::core::{authority, materialization, sources};

struct View {
    recipe: Recipe,
    artifact: ArtifactHandle,
    mirrors: Vec<MirrorBinding>,
}

impl Session {
    pub(in crate::core) fn mirror(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        target: &BindingTarget,
        destination: &Path,
        verification: &VerificationRequest,
    ) -> Result<MirrorBinding> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let view = view(&self.read_index()?, &context.id, target)?;
        authority::require_source(
            &context,
            &view.artifact.inputs.source_id,
            &view.artifact.inputs.subdirectory,
            Action::Mirror,
        )?;
        let destination = external_path(destination)?;
        authority::require_destination(
            &context,
            &view.artifact.inputs.source_id,
            &view.artifact.inputs.subdirectory,
            Action::Mirror,
            &destination,
        )?;
        if authority::path_contains(self.layout.root(), &destination)
            || authority::path_contains(&destination, self.layout.root())
        {
            return Err(Error::new(
                ErrorKind::Conflict,
                "mirror destination overlaps the central store",
            ));
        }
        let previous = view
            .mirrors
            .iter()
            .find(|mirror| Path::new(&mirror.directory.path) == destination)
            .cloned();
        for path in super::super::mirrors::records(&self.read_index()?)?.keys() {
            let path = Path::new(path);
            if (authority::path_contains(path, &destination)
                || authority::path_contains(&destination, path))
                && !(path == destination && previous.is_some())
            {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "mirror destination overlaps another binding",
                ));
            }
        }
        if destination.exists() && previous.is_none() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "mirror destination is not owned by this binding",
            ));
        }
        if let Some(previous) = &previous {
            materialization::check(
                &destination,
                &previous.directory.entries,
                !destination.exists(),
            )
            .map_err(|_| {
                Error::new(
                    ErrorKind::Conflict,
                    "mirror contains modified or unowned entries",
                )
            })?;
        }
        let identity = sources::identify(&view.recipe.source, Path::new(&context.root))?;
        let mut access = self.access_source(
            root,
            marker,
            &identity,
            &view.artifact.inputs.subdirectory,
            Action::Mirror,
            verification,
        )?;
        self.guard_dependencies(&mut access, &view.artifact.inputs)?;
        let repository = sources::GitRepository::open(&access.repository, &identity)?;
        let retained =
            access.source.history.contains_key(&view.artifact.id)
                || access.source.current.values().any(|current| {
                    current.artifact.id == view.artifact.id && current.archive_retained
                });
        let record = view.artifact.content_record();
        let bytes = if retained {
            self.read_archive(&access, &record, access.policy.verify_content)?
        } else {
            let exported = self.reconstruct(&mut access, &repository, &record)?;
            if exported.tree_digest != record.tree_digest
                || exported.lfs_objects != record.inputs.lfs_objects
            {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "reconstructed mirror tree changed",
                ));
            }
            exported.bytes
        };
        let parent = destination
            .parent()
            .ok_or_else(|| Error::new(ErrorKind::Config, "mirror destination has no parent"))?;
        if !parent.is_dir() {
            return Err(Error::new(
                ErrorKind::Config,
                "mirror destination parent must exist",
            ));
        }
        let prepared = materialization::prepare(
            parent,
            &bytes,
            &record.tree_digest,
            access.policy.verify_content,
        )?;
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
                "application changed during mirror preparation",
            ));
        }
        authority::require_destination(
            &access.context,
            &record.inputs.source_id,
            &record.inputs.subdirectory,
            Action::Mirror,
            &destination,
        )?;
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
        let path = destination
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::Config, "mirror path is not UTF-8"))?
            .to_owned();
        let mirror = MirrorBinding {
            artifact: record.clone(),
            directory: OwnedDirectory {
                path: path.clone(),
                tree_digest: record.tree_digest.clone(),
                generation: app.data_generation,
                entries: prepared.entries,
            },
        };
        let mirrors = mirrors_mut(&mut snapshot.central, &context.id, target)?;
        mirrors.retain(|old| old.directory.path != path);
        mirrors.push(mirror.clone());
        let staged = prepared
            .staging
            .path()
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "invalid mirror staging path"))?
            .to_owned();
        let change = MirrorChange::new(path, Some(staged), previous, Some(mirror.clone()))?;
        self.generations()
            .commit_mirror(snapshot.central.generation, snapshot.central, change)?;
        self.reconcile_pins(&access)?;
        Ok(mirror)
    }

    pub(in crate::core) fn mirror_path(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        target: &BindingTarget,
        destination: &Path,
        verification: &VerificationRequest,
    ) -> Result<std::path::PathBuf> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let view = view(&self.read_index()?, &context.id, target)?;
        let destination = external_path(destination)?;
        let mirror = view
            .mirrors
            .iter()
            .find(|mirror| Path::new(&mirror.directory.path) == destination)
            .ok_or_else(Error::not_found)?;
        authority::require_destination(
            &context,
            &mirror.artifact.inputs.source_id,
            &mirror.artifact.inputs.subdirectory,
            Action::Inspect,
            &destination,
        )
        .map_err(|_| Error::not_found())?;
        let identity = sources::identify(&view.recipe.source, Path::new(&context.root))?;
        let mut access = self.inspect_source(
            root,
            marker,
            &identity,
            &mirror.artifact.inputs.subdirectory,
            verification,
        )?;
        self.guard_dependencies(&mut access, &mirror.artifact.inputs)?;
        let repository = sources::GitRepository::open(&access.repository, &identity)?;
        if !destination.is_dir() {
            return Err(Error::not_found());
        }
        if access.policy.verify_content
            && materialization::check(&destination, &mirror.directory.entries, false)?
                != mirror.artifact.tree_digest
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "mirror does not match its authenticated artifact",
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
                "application changed during mirror lookup",
            ));
        }
        authority::require_destination(
            &access.context,
            &mirror.artifact.inputs.source_id,
            &mirror.artifact.inputs.subdirectory,
            Action::Inspect,
            &destination,
        )?;
        repository.verify_origin()?;
        Ok(destination)
    }

    pub(in crate::core) fn remove_mirror(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        target: &BindingTarget,
        destination: &Path,
    ) -> Result<()> {
        let context = self.inspect_context(root, marker)?;
        let _binding = Lock::acquire(
            &self.layout,
            &format!("binding-{}", context.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let view = view(&self.read_index()?, &context.id, target)?;
        let destination = external_path(destination)?;
        let mirror = view
            .mirrors
            .iter()
            .find(|mirror| Path::new(&mirror.directory.path) == destination)
            .ok_or_else(Error::not_found)?;
        authority::require_destination(
            &context,
            &mirror.artifact.inputs.source_id,
            &mirror.artifact.inputs.subdirectory,
            Action::Remove,
            &destination,
        )
        .map_err(|_| Error::not_found())?;
        let identity = sources::identify(&view.recipe.source, Path::new(&context.root))?;
        let mut access = self.access_source(
            root,
            marker,
            &identity,
            &mirror.artifact.inputs.subdirectory,
            Action::Remove,
            &VerificationRequest { content: None },
        )?;
        self.guard_dependencies(&mut access, &mirror.artifact.inputs)?;
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
                "application changed during mirror removal",
            ));
        }
        authority::require_destination(
            &access.context,
            &mirror.artifact.inputs.source_id,
            &mirror.artifact.inputs.subdirectory,
            Action::Remove,
            &destination,
        )?;
        repository.verify_origin()?;
        let mirrors = mirrors_mut(&mut snapshot.central, &context.id, target)?;
        mirrors.retain(|mirror| Path::new(&mirror.directory.path) != destination);
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

fn view(central: &CentralIndex, app: &str, target: &BindingTarget) -> Result<View> {
    match target {
        BindingTarget::Unit { name } => {
            let unit = central
                .units
                .get(&super::installations::binding_id(app, name))
                .filter(|unit| unit.app_id == app)
                .ok_or_else(Error::not_found)?;
            Ok(View {
                recipe: unit.recipe.clone(),
                artifact: unit.artifact.clone(),
                mirrors: unit.mirrors.clone(),
            })
        }
        BindingTarget::Materialization { id } => {
            let binding = central
                .materializations
                .get(id)
                .filter(|binding| binding.app_id == app)
                .ok_or_else(Error::not_found)?;
            Ok(View {
                recipe: binding.recipe.clone(),
                artifact: binding.artifact.clone(),
                mirrors: binding.mirrors.clone(),
            })
        }
    }
}
fn mirrors_mut<'a>(
    central: &'a mut CentralIndex,
    app: &str,
    target: &BindingTarget,
) -> Result<&'a mut Vec<MirrorBinding>> {
    match target {
        BindingTarget::Unit { name } => central
            .units
            .get_mut(&super::installations::binding_id(app, name))
            .filter(|unit| unit.app_id == app)
            .map(|unit| &mut unit.mirrors),
        BindingTarget::Materialization { id } => central
            .materializations
            .get_mut(id)
            .filter(|binding| binding.app_id == app)
            .map(|binding| &mut binding.mirrors),
    }
    .ok_or_else(Error::not_found)
}
