use super::*;
use crate::core::sources::GitRepository;
use std::collections::{BTreeMap, BTreeSet};

impl Session {
    /// Lock sources referenced by descriptors this publication retires, even
    /// when the incoming tree no longer contains those dependencies. This only
    /// maintains Saucepan refs; it does not grant source access or fetch bytes.
    pub(in crate::core) fn reserve_retired_sources(
        &self,
        access: &mut super::access::SourceAccess,
        next: &SourceIndex,
    ) -> Result<()> {
        let retained: BTreeSet<_> = artifacts(next).map(|artifact| &artifact.id).collect();
        let mut origins = BTreeMap::new();
        for dependency in artifacts(&access.source)
            .filter(|artifact| !retained.contains(&artifact.id))
            .flat_map(|artifact| &artifact.inputs.dependencies)
        {
            if dependency.source_id != access.source.id
                && !access.dependencies.contains_key(&dependency.source_id)
                && origins
                    .insert(dependency.source_id.clone(), dependency.origin.clone())
                    .is_some_and(|previous| previous != dependency.origin)
            {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "retired dependency origins conflict",
                ));
            }
        }
        if origins.is_empty() {
            return Ok(());
        }
        let ids: BTreeSet<_> = std::iter::once(access.source.id.clone())
            .chain(access.dependencies.keys().cloned())
            .chain(access.maintenance.keys().cloned())
            .chain(origins.keys().cloned())
            .collect();
        drop(std::mem::take(&mut access.locks));
        let deadline = Instant::now() + Duration::from_secs(5);
        for id in ids {
            access.locks.push(Lock::acquire(
                &self.layout,
                &format!("source-{id}"),
                deadline,
            )?);
        }
        let _central = Lock::acquire(&self.layout, "central", deadline)?;
        self.generations().recover()?;
        let snapshot = self.generations().read()?;
        self.revalidate_source_state(access, &snapshot)?;
        for (id, origin) in origins {
            let source = snapshot.sources.get(&id).ok_or_else(|| {
                Error::new(ErrorKind::Integrity, "retired dependency source is missing")
            })?;
            if source.origin != origin {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "retired dependency origin changed",
                ));
            }
            access.maintenance.insert(id, source.clone());
        }
        // Verify every repository that publication will reconcile before the
        // central index is changed, including dependencies no longer acquired.
        self.revalidate_source(access, &snapshot)
    }

    /// Caller holds this source lock and the central commit lock. Extra refs
    /// left by a crash are harmless and are retired at the next reconciliation.
    pub(super) fn reconcile_pins(&self, access: &super::access::SourceAccess) -> Result<()> {
        let snapshot = self.generations().read()?;
        let ids: BTreeSet<_> = std::iter::once(&access.source.id)
            .chain(access.dependencies.keys())
            .chain(access.maintenance.keys())
            .collect();
        for source_id in ids {
            let source = snapshot
                .sources
                .get(source_id)
                .ok_or_else(Error::not_found)?;
            let identity = SourceIdentity {
                schema_version: SCHEMA_VERSION,
                backend: Backend::Git,
                id: source.id.clone(),
                origin: source.origin.clone(),
                locator: source.origin.clone(),
            };
            let mut expected = BTreeMap::new();
            for artifact in snapshot.sources.values().flat_map(|source| {
                source
                    .current
                    .values()
                    .map(|current| &current.artifact)
                    .chain(source.history.values().map(|history| &history.artifact))
            }) {
                collect(&mut expected, &source.id, artifact)?;
            }
            for unit in snapshot.central.units.values() {
                collect(&mut expected, &source.id, &unit.artifact.content_record())?;
                for mirror in &unit.mirrors {
                    collect(&mut expected, &source.id, &mirror.artifact)?;
                }
            }
            for binding in snapshot.central.materializations.values() {
                collect(
                    &mut expected,
                    &source.id,
                    &binding.artifact.content_record(),
                )?;
                for mirror in &binding.mirrors {
                    collect(&mut expected, &source.id, &mirror.artifact)?;
                }
            }
            let repository = self
                .layout
                .path(Path::new(&format!("sources/{source_id}/repo.git")))?;
            GitRepository::open(&repository, &identity)?.reconcile_pins(&expected)?;
        }
        Ok(())
    }
}
fn artifacts(source: &SourceIndex) -> impl Iterator<Item = &ContentArtifact> {
    source
        .current
        .values()
        .map(|current| &current.artifact)
        .chain(source.history.values().map(|history| &history.artifact))
}
fn collect(
    expected: &mut BTreeMap<String, String>,
    source: &str,
    artifact: &ContentArtifact,
) -> Result<()> {
    if artifact.inputs.source_id == source {
        insert(
            expected,
            format!("refs/saucepan/artifacts/{}", artifact.id),
            &artifact.inputs.commit,
        )?;
    }
    for dependency in &artifact.inputs.dependencies {
        if dependency.source_id == source {
            let id = utils::hex(&Sha256::digest(utils::frame(
                b"saucepan/dependency-pin/v1",
                &[
                    artifact.id.as_bytes(),
                    dependency.path.as_bytes(),
                    dependency.source_id.as_bytes(),
                    dependency.commit.as_bytes(),
                ],
            )));
            insert(
                expected,
                format!("refs/saucepan/dependencies/{id}"),
                &dependency.commit,
            )?;
        }
    }
    Ok(())
}
fn insert(expected: &mut BTreeMap<String, String>, reference: String, commit: &str) -> Result<()> {
    if expected
        .insert(reference, commit.into())
        .is_some_and(|previous| previous != commit)
    {
        return Err(Error::new(
            ErrorKind::Integrity,
            "conflicting durable artifact references",
        ));
    }
    Ok(())
}
