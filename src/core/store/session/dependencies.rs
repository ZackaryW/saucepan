use super::access::SourceAccess;
use super::*;
use crate::core::{authority, policies, snapshots, sources};
use std::collections::BTreeSet;

impl Session {
    /// Adding a discovered source releases and reacquires the complete sorted
    /// source lock set. Every previously observed generation is then checked;
    /// concurrent publication wins and this preparation returns Busy.
    fn dependency_repository(
        &self,
        access: &mut SourceAccess,
        identity: &SourceIdentity,
        selection: &str,
        acquire: bool,
    ) -> Result<sources::GitRepository> {
        authority::require_source(&access.context, &identity.id, selection, access.action)?;
        let mut ids: BTreeSet<_> = access.dependencies.keys().cloned().collect();
        ids.insert(access.source.id.clone());
        if ids.insert(identity.id.clone()) {
            drop(std::mem::take(&mut access.locks));
            let deadline = Instant::now() + Duration::from_secs(5);
            for id in ids {
                access.locks.push(Lock::acquire(
                    &self.layout,
                    &format!("source-{id}"),
                    deadline,
                )?);
            }
        }
        {
            let _central = Lock::acquire(
                &self.layout,
                "central",
                Instant::now() + Duration::from_secs(5),
            )?;
            self.generations().recover()?;
            let snapshot = self.generations().read()?;
            self.revalidate_source_state(access, &snapshot)?;
            authority::require_source(
                &self.context_in(
                    &snapshot.central,
                    Path::new(&access.context.root),
                    access.marker.as_ref(),
                )?,
                &identity.id,
                selection,
                access.action,
            )?;
            let relative = format!("sources/{}", identity.id);
            let source = if let Some(source) = snapshot.sources.get(&identity.id) {
                if source.origin != identity.origin {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "dependency origin does not match encrypted source",
                    ));
                }
                source.clone()
            } else {
                if !acquire {
                    return Err(Error::not_found());
                }
                if self.layout.path(Path::new(&relative))?.exists() {
                    return Err(Error::new(
                        ErrorKind::Conflict,
                        "unrecognized dependency directory already exists",
                    ));
                }
                let source = SourceIndex {
                    schema_version: SCHEMA_VERSION,
                    id: identity.id.clone(),
                    origin: identity.origin.clone(),
                    generation: 0,
                    sequence: 0,
                    policy: CachePolicy::default(),
                    current: Default::default(),
                    history: Default::default(),
                };
                self.generations().commit(
                    snapshot.central.generation,
                    snapshot.central,
                    vec![source],
                )?;
                self.generations()
                    .read()?
                    .sources
                    .remove(&identity.id)
                    .ok_or_else(Error::not_found)?
            };
            let policy = policies::evaluate(
                &source.policy,
                access.context.require_verification,
                &access.verification,
            )?;
            access.policy.verify_content |= policy.verify_content;
            if identity.id != access.source.id {
                access.dependencies.insert(identity.id.clone(), source);
            }
            access
                .dependency_scopes
                .insert((identity.id.clone(), selection.into()));
            if acquire
                && !self
                    .layout
                    .path(Path::new(&format!("{relative}/archives")))?
                    .exists()
            {
                self.layout
                    .create_directory(Path::new(&format!("{relative}/archives")))?;
            }
        }
        let repository = self
            .layout
            .path(Path::new(&format!("sources/{}/repo.git", identity.id)))?;
        if !repository.exists() {
            if !acquire {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "recorded dependency repository is missing",
                ));
            }
            let staged = access
                .staging
                .path()
                .join(format!("dependency-{}.git", identity.id));
            sources::GitRepository::clone_into(&staged, identity)?;
            let _central = Lock::acquire(
                &self.layout,
                "central",
                Instant::now() + Duration::from_secs(5),
            )?;
            self.generations().recover()?;
            self.revalidate_source_state(access, &self.generations().read()?)?;
            if repository.exists() {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "dependency publication destination exists",
                ));
            }
            std::fs::rename(staged, &repository).map_err(|_| {
                Error::new(ErrorKind::Internal, "cannot publish dependency repository")
            })?;
        }
        sources::GitRepository::open(&repository, identity)
    }

    pub(in crate::core) fn prepare_export(
        &self,
        access: &mut SourceAccess,
        repository: &sources::GitRepository,
        commit: &str,
        selection: &str,
    ) -> Result<(snapshots::PreparedExport, Vec<Dependency>)> {
        let mut ancestors = vec![(access.source.id.clone(), commit.to_owned())];
        let mut count = 0;
        access.pins.push(repository.temporary_pin(commit)?);
        self.prepare_node(
            access,
            repository,
            commit,
            selection,
            &mut ancestors,
            &mut count,
        )
    }
    fn prepare_node(
        &self,
        access: &mut SourceAccess,
        repository: &sources::GitRepository,
        commit: &str,
        selection: &str,
        ancestors: &mut Vec<(String, String)>,
        count: &mut usize,
    ) -> Result<(snapshots::PreparedExport, Vec<Dependency>)> {
        let mut prepared =
            snapshots::prepare_export(repository, commit, selection, access.staging.path())?;
        let mut dependencies = vec![];
        for child in std::mem::take(&mut prepared.submodules) {
            *count += 1;
            if *count > 128 || ancestors.len() >= 16 {
                return Err(Error::new(
                    ErrorKind::Source,
                    "submodule count or depth limit exceeded",
                ));
            }
            let identity = sources::identify(
                &SourceLocator {
                    backend: Backend::Git,
                    origin: child.locator,
                },
                Path::new(&access.context.root),
            )?;
            if ancestors.contains(&(identity.id.clone(), child.commit.clone())) {
                return Err(Error::new(ErrorKind::Source, "cyclic submodule inputs"));
            }
            let repository =
                self.dependency_repository(access, &identity, &child.selection, true)?;
            let resolved = repository.resolve(&Revision::Commit(child.commit.clone()))?;
            if resolved != child.commit {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "submodule did not resolve to its recorded gitlink",
                ));
            }
            access.pins.push(repository.temporary_pin(&resolved)?);
            ancestors.push((identity.id.clone(), resolved.clone()));
            let (export, nested) = self.prepare_node(
                access,
                &repository,
                &resolved,
                &child.selection,
                ancestors,
                count,
            )?;
            ancestors.pop();
            let exported = export.finish_nested(&repository, &resolved, access.staging.path())?;
            prepared.include(&child.path, exported)?;
            dependencies.push(Dependency {
                path: child.path.clone(),
                source_id: identity.id,
                origin: identity.origin,
                commit: resolved,
                subdirectory: child.selection,
            });
            for mut dependency in nested {
                dependency.path = join(&child.path, &dependency.path);
                dependencies.push(dependency);
            }
        }
        // Stable path order preserves ancestor-before-descendant when a
        // selection crosses several gitlinks mounted at the same ZIP root.
        dependencies.sort_by(|a, b| a.path.cmp(&b.path));
        repository.verify_origin()?;
        Ok((prepared, dependencies))
    }

    pub(in crate::core) fn guard_dependencies(
        &self,
        access: &mut SourceAccess,
        inputs: &ResolvedInputs,
    ) -> Result<()> {
        for dependency in &inputs.dependencies {
            let identity = SourceIdentity {
                schema_version: SCHEMA_VERSION,
                backend: Backend::Git,
                id: dependency.source_id.clone(),
                origin: dependency.origin.clone(),
                locator: dependency.origin.clone(),
            };
            self.dependency_repository(access, &identity, &dependency.subdirectory, false)?
                .verify_origin()?;
        }
        self.verify_lfs_origins(access, inputs)
    }
    pub(super) fn verify_lfs_origins(
        &self,
        access: &SourceAccess,
        inputs: &ResolvedInputs,
    ) -> Result<()> {
        let mut origins = BTreeSet::new();
        for object in &inputs.lfs_objects {
            let dependency = inputs
                .dependencies
                .iter()
                .enumerate()
                .filter(|(_, dependency)| {
                    dependency.path == "."
                        || object
                            .path
                            .strip_prefix(&dependency.path)
                            .is_some_and(|rest| rest.starts_with('/'))
                })
                .max_by_key(|(index, dependency)| {
                    (
                        if dependency.path == "." {
                            0
                        } else {
                            dependency.path.split('/').count()
                        },
                        *index,
                    )
                })
                .map(|(_, dependency)| dependency);
            origins.insert(
                dependency.map_or((&inputs.source_id, &inputs.commit), |dependency| {
                    (&dependency.source_id, &dependency.commit)
                }),
            );
        }
        for (id, commit) in origins {
            let source = if id == &access.source.id {
                &access.source
            } else {
                access.dependencies.get(id).ok_or_else(|| {
                    Error::new(ErrorKind::Integrity, "LFS dependency was not gated")
                })?
            };
            let identity = SourceIdentity {
                schema_version: SCHEMA_VERSION,
                backend: Backend::Git,
                id: id.clone(),
                origin: source.origin.clone(),
                locator: source.origin.clone(),
            };
            let path = self
                .layout
                .path(Path::new(&format!("sources/{id}/repo.git")))?;
            sources::GitRepository::open(&path, &identity)?
                .lfs_download(commit, access.staging.path())?;
        }
        Ok(())
    }
    pub(in crate::core) fn reconstruct(
        &self,
        access: &mut SourceAccess,
        repository: &sources::GitRepository,
        record: &ContentArtifact,
    ) -> Result<snapshots::Exported> {
        self.guard_dependencies(access, &record.inputs)?;
        let (prepared, dependencies) = self.prepare_export(
            access,
            repository,
            &record.inputs.commit,
            &record.inputs.subdirectory,
        )?;
        if dependencies != record.inputs.dependencies
            || prepared.lfs_objects != record.inputs.lfs_objects
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "reconstructed dependency inputs differ from the authenticated artifact",
            ));
        }
        let exported = prepared.finish(repository, &record.inputs.commit, access.staging.path())?;
        if exported.tree_digest != record.tree_digest {
            return Err(Error::new(
                ErrorKind::Integrity,
                "reconstructed artifact tree changed",
            ));
        }
        Ok(exported)
    }
}
fn join(parent: &str, child: &str) -> String {
    match (parent, child) {
        (".", child) => child.into(),
        (parent, ".") => parent.into(),
        _ => format!("{parent}/{child}"),
    }
}
