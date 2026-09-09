use super::*;
use crate::core::sources::GitRepository;
use std::collections::BTreeMap;

impl Session {
    /// Caller holds this source lock and the central commit lock. Extra refs
    /// left by a crash are harmless and are retired at the next reconciliation.
    pub(super) fn reconcile_pins(&self, access: &super::access::SourceAccess) -> Result<()> {
        let snapshot = self.generations().read()?;
        for source_id in std::iter::once(&access.source.id).chain(access.dependencies.keys()) {
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
