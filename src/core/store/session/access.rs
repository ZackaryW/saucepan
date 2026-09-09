use super::*;

pub(in crate::core) struct SourceAccess {
    pub source: SourceIndex,
    pub policy: EffectivePolicy,
    pub repository: std::path::PathBuf,
    pub staging: tempfile::TempDir,
    pub(super) context: Registration,
    pub(super) marker: Option<Marker>,
    pub(super) selection: String,
    pub(super) action: Action,
    pub(super) locks: Vec<Lock>,
    pub(super) dependencies: std::collections::BTreeMap<String, SourceIndex>,
    pub(super) dependency_scopes: std::collections::BTreeSet<(String, String)>,
    pub(super) verification: VerificationRequest,
    pub pins: Vec<crate::core::sources::GitPin>,
}
impl Session {
    #[cfg(test)]
    pub(in crate::core) fn begin_source(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        identity: &SourceIdentity,
        selection: &str,
        verification: &VerificationRequest,
    ) -> Result<SourceAccess> {
        self.access_source(
            root,
            marker,
            identity,
            selection,
            Action::Setup,
            verification,
        )
    }
    pub(in crate::core) fn inspect_source(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        identity: &SourceIdentity,
        selection: &str,
        verification: &VerificationRequest,
    ) -> Result<SourceAccess> {
        self.access_source(
            root,
            marker,
            identity,
            selection,
            Action::Inspect,
            verification,
        )
    }
    pub(in crate::core) fn access_source(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        identity: &SourceIdentity,
        selection: &str,
        action: Action,
        verification: &VerificationRequest,
    ) -> Result<SourceAccess> {
        let context = self.inspect_context(root, marker)?;
        crate::core::authority::require_source(&context, &identity.id, selection, action)?;
        let lock = Lock::acquire(
            &self.layout,
            &format!("source-{}", identity.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let snapshot = self.generations().read()?;
        let context = self.context_in(&snapshot.central, root, marker)?;
        crate::core::authority::require_source(&context, &identity.id, selection, action)?;
        let policy = crate::core::policies::evaluate(
            &snapshot
                .sources
                .get(&identity.id)
                .map(|source| source.policy.clone())
                .unwrap_or_default(),
            context.require_verification,
            verification,
        )?;
        let relative = format!("sources/{}", identity.id);
        let source = if let Some(source) = snapshot.sources.get(&identity.id) {
            if source.origin != identity.origin {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "encrypted source origin does not match request",
                ));
            }
            source.clone()
        } else {
            if action != Action::Setup {
                return Err(Error::not_found());
            }
            if self.layout.path(Path::new(&relative))?.exists() {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "unrecognized source directory already exists",
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
                .ok_or_else(|| {
                    Error::new(ErrorKind::Integrity, "source enrollment was not published")
                })?
        };
        let archives = format!("{relative}/archives");
        if !self.layout.path(Path::new(&archives))?.exists() {
            self.layout.create_directory(Path::new(&archives))?;
        }
        let staging = tempfile::Builder::new()
            .prefix("acquire-")
            .tempdir_in(self.layout.path(Path::new("transactions"))?)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot create acquisition staging"))?;
        Ok(SourceAccess {
            source,
            policy,
            repository: self
                .layout
                .path(Path::new(&format!("{relative}/repo.git")))?,
            staging,
            context,
            marker: marker.cloned(),
            selection: selection.into(),
            action,
            locks: vec![lock],
            dependencies: Default::default(),
            dependency_scopes: Default::default(),
            verification: verification.clone(),
            pins: vec![],
        })
    }
    pub(super) fn revalidate_source(
        &self,
        access: &SourceAccess,
        snapshot: &super::super::transactions::Snapshot,
    ) -> Result<()> {
        self.revalidate_source_state(access, snapshot)?;
        // Protected returns and publication verify the complete repository set.
        // Discovery checks each repository when it is used and revalidates all
        // index generations, without repeatedly spawning Git for every ancestor.
        for (id, source) in &access.dependencies {
            let identity = SourceIdentity {
                schema_version: SCHEMA_VERSION,
                backend: Backend::Git,
                id: id.clone(),
                origin: source.origin.clone(),
                locator: source.origin.clone(),
            };
            let repository = self
                .layout
                .path(Path::new(&format!("sources/{id}/repo.git")))?;
            if !repository.is_dir() {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "recorded dependency repository is missing",
                ));
            }
            crate::core::sources::GitRepository::open(&repository, &identity)?;
        }
        Ok(())
    }
    pub(super) fn revalidate_source_state(
        &self,
        access: &SourceAccess,
        snapshot: &super::super::transactions::Snapshot,
    ) -> Result<()> {
        let current = self.context_in(
            &snapshot.central,
            Path::new(&access.context.root),
            access.marker.as_ref(),
        )?;
        if current.revision != access.context.revision {
            return Err(Error::new(
                ErrorKind::Authority,
                "application authority changed during acquisition",
            ));
        }
        crate::core::authority::require_source(
            &current,
            &access.source.id,
            &access.selection,
            access.action,
        )?;
        for (id, selection) in &access.dependency_scopes {
            crate::core::authority::require_source(&current, id, selection, access.action)?;
        }
        for (id, dependency) in &access.dependencies {
            let selected = snapshot.sources.get(id).ok_or_else(Error::not_found)?;
            if selected.generation != dependency.generation
                || selected.origin != dependency.origin
                || selected.policy != dependency.policy
            {
                return Err(Error::new(
                    ErrorKind::Busy,
                    "dependency changed during acquisition; retry",
                ));
            }
        }
        let source = snapshot
            .sources
            .get(&access.source.id)
            .ok_or_else(Error::not_found)?;
        if source.generation != access.source.generation || source.policy != access.source.policy {
            return Err(Error::new(
                ErrorKind::Busy,
                "source changed during acquisition; retry",
            ));
        }
        Ok(())
    }
    pub(in crate::core) fn publish_repository(
        &self,
        access: &SourceAccess,
        staged: &Path,
    ) -> Result<()> {
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        self.revalidate_source(access, &self.generations().read()?)?;
        let staging_root = std::fs::canonicalize(access.staging.path())
            .map_err(|_| Error::new(ErrorKind::Integrity, "acquisition staging is missing"))?;
        let staged = std::fs::canonicalize(staged)
            .map_err(|_| Error::new(ErrorKind::Integrity, "staged repository is missing"))?;
        if staged.parent() != Some(staging_root.as_path()) {
            return Err(Error::new(
                ErrorKind::Integrity,
                "repository is outside owned staging",
            ));
        }
        let destination = self
            .layout
            .path(Path::new(&format!("sources/{}/repo.git", access.source.id)))?;
        if destination.exists() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "repository publication destination already exists",
            ));
        }
        std::fs::rename(staged, destination)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot publish staged repository"))
    }
    pub(in crate::core) fn validate_archive(
        &self,
        access: &SourceAccess,
        artifact: &ContentArtifact,
        verify: bool,
    ) -> Result<()> {
        let path = Path::new("sources")
            .join(&access.source.id)
            .join("archives")
            .join(format!("{}.zip", artifact.id));
        let resolved = self.layout.path(&path)?;
        let metadata = std::fs::metadata(resolved)
            .map_err(|_| Error::new(ErrorKind::Integrity, "committed archive is missing"))?;
        if !metadata.is_file() || metadata.len() != artifact.archive_size {
            return Err(Error::new(
                ErrorKind::Integrity,
                "committed archive size mismatch",
            ));
        }
        if !artifact.inputs.lfs_objects.is_empty() {
            self.read_archive(access, artifact, verify)?;
        } else if verify {
            let bytes = self.layout.read(&path, 64 * 1024 * 1024)?;
            if utils::hex(&Sha256::digest(bytes)) != artifact.archive_digest {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "committed archive digest mismatch",
                ));
            }
        }
        Ok(())
    }
    pub(in crate::core) fn read_archive(
        &self,
        access: &SourceAccess,
        artifact: &ContentArtifact,
        verify: bool,
    ) -> Result<Vec<u8>> {
        self.verify_lfs_origins(access, &artifact.inputs)?;
        // Readers take source -> archive, and release the lease before a
        // central recency commit. Recovery can then wait without lock inversion.
        let _lease = Lock::shared(
            &self.layout,
            &format!("archive-{}-{}", access.source.id, artifact.id),
            Instant::now() + Duration::from_secs(5),
        )?;
        let path = Path::new("sources")
            .join(&access.source.id)
            .join("archives")
            .join(format!("{}.zip", artifact.id));
        let bytes = self.layout.read(&path, 64 * 1024 * 1024)?;
        if bytes.len() as u64 != artifact.archive_size
            || (verify && utils::hex(&Sha256::digest(&bytes)) != artifact.archive_digest)
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "committed archive content mismatch",
            ));
        }
        crate::core::snapshots::validate_lfs(&bytes, &artifact.inputs.lfs_objects)?;
        Ok(bytes)
    }
    pub(in crate::core) fn trim_history(
        &self,
        access: &SourceAccess,
        source: &mut SourceIndex,
    ) -> Result<()> {
        if !access.policy.retain {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while source.history.len() > access.policy.history_limit {
            let mut candidates: Vec<_> = source
                .history
                .iter()
                // An outgoing current must be admitted, rather than silently
                // discarded when all prior historical entries have readers.
                .filter(|(id, _)| {
                    access.source.history.contains_key(*id) || access.policy.history_limit == 0
                })
                .collect();
            candidates.sort_by_key(|(id, h)| (h.last_used, h.created, *id));
            let mut victim = None;
            for (id, _) in candidates {
                match Lock::acquire(
                    &self.layout,
                    &format!("archive-{}-{id}", source.id),
                    Instant::now(),
                ) {
                    Ok(_lease) => {
                        victim = Some(id.clone());
                        break;
                    }
                    Err(e) if e.kind == ErrorKind::Busy => {}
                    Err(e) => return Err(e),
                }
            }
            if let Some(id) = victim {
                source.history.remove(&id);
            } else if Instant::now() >= deadline {
                return Err(Error::new(
                    ErrorKind::Busy,
                    "historical archives have active readers; retry",
                ));
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        // The caller still holds the source lock, so no new supported reader
        // can enter before publication reserves the retiring archive leases.
        Ok(())
    }
    pub(in crate::core) fn commit_source(
        &self,
        access: &SourceAccess,
        source: SourceIndex,
        archives: Vec<(String, Vec<u8>)>,
    ) -> Result<()> {
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let snapshot = self.generations().read()?;
        self.revalidate_source(access, &snapshot)?;
        self.generations().commit_archives(
            snapshot.central.generation,
            snapshot.central,
            vec![source],
            archives,
        )?;
        self.reconcile_pins(access)
    }
}
