//! The operation coordinator owns ordering. Preflight is deliberately pure;
//! successful validation is not an authorization capability.
use super::{models::*, recipes, snapshots, sources, store::Session};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) fn preflight(recipe: &Recipe) -> Result<()> {
    recipes::validate(recipe)
}

pub(super) fn acquire(
    session: &Session,
    root: &Path,
    marker: Option<&Marker>,
    recipe: &Recipe,
    verification: &VerificationRequest,
) -> Result<ArtifactHandle> {
    acquire_for_action(session, root, marker, recipe, verification, Action::Setup)
}

pub(super) fn acquire_for_action(
    session: &Session,
    root: &Path,
    marker: Option<&Marker>,
    recipe: &Recipe,
    verification: &VerificationRequest,
    action: Action,
) -> Result<ArtifactHandle> {
    let context = session.inspect_context(root, marker)?;
    preflight(recipe)?;
    let identity = sources::identify(&recipe.source, Path::new(&context.root))?;
    let mut access = session.access_source(
        root,
        marker,
        &identity,
        &recipe.export.subdirectory,
        action,
        verification,
    )?;
    if !access.repository.exists() {
        let staged = access.staging.path().join("repo.git");
        sources::GitRepository::clone_into(&staged, &identity)?;
        session.publish_repository(&access, &staged)?;
    }
    let repository = sources::GitRepository::open(&access.repository, &identity)?;
    let commit = repository.resolve(&recipe.revision)?;
    // Fixed commits also need a temporary root during export, even when they
    // were already present locally and resolve() did not fetch a moving ref.
    repository.pin("refs/saucepan/resolved", &commit)?;
    let (stream_id, descriptor) = recipes::stream(recipe, &identity.id)?;
    let mut source = access.source.clone();
    let existing = source.current.get(&stream_id).cloned();
    let (prepared, inputs) = if let Some(current) = &existing
        && current.artifact.inputs.commit == commit
        && current.artifact.inputs.subdirectory == recipe.export.subdirectory
        && current.artifact.inputs.export_version == recipe.export.version
        && current.archive_retained
        && access.policy.retain
    {
        session.guard_dependencies(&mut access, &current.artifact.inputs)?;
        (None, current.artifact.inputs.clone())
    } else {
        let (prepared, dependencies) = session.prepare_export(
            &mut access,
            &repository,
            &commit,
            &recipe.export.subdirectory,
        )?;
        let inputs = ResolvedInputs {
            source_id: identity.id.clone(),
            stream_id: stream_id.clone(),
            subdirectory: recipe.export.subdirectory.clone(),
            commit,
            dependencies,
            lfs_objects: prepared.lfs_objects.clone(),
            export_version: recipe.export.version,
        };
        (Some(prepared), inputs)
    };
    let policy = access.policy.clone();
    let mut archives = vec![];
    let artifact = if let Some(current) = &existing
        && current.artifact.inputs == inputs
        && current.archive_retained
        && policy.retain
    {
        session.validate_archive(&access, &current.artifact, policy.verify_content)?;
        current.artifact.clone()
    } else {
        if policy.retain
            && let Some(current) = &existing
            && current.artifact.inputs != inputs
        {
            session.guard_dependencies(&mut access, &current.artifact.inputs)?;
            if current.archive_retained {
                session.validate_archive(&access, &current.artifact, true)?;
            } else {
                let restored = session.reconstruct(&mut access, &repository, &current.artifact)?;
                if restored.tree_digest != current.artifact.tree_digest
                    || restored.lfs_objects != current.artifact.inputs.lfs_objects
                {
                    return Err(Error::new(
                        ErrorKind::Integrity,
                        "outgoing reconstruction does not match its committed tree",
                    ));
                }
                let previous = source
                    .current
                    .get_mut(&stream_id)
                    .expect("current came from this map");
                previous.artifact.archive_digest = hash(&restored.bytes);
                previous.artifact.archive_size = restored.bytes.len() as u64;
                previous.archive_retained = true;
                archives.push((
                    archive_path(&identity.id, &previous.artifact.id),
                    restored.bytes,
                ));
            }
        }
        let exported = prepared
            .expect("changed or uncached inputs have a prepared export")
            .finish(&repository, &inputs.commit, access.staging.path())?;
        if let Some(current) = &existing
            && current.artifact.inputs == inputs
            && current.artifact.tree_digest != exported.tree_digest
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "reconstructed tree does not match committed content",
            ));
        }
        // A stream selector is not content: identical pinned/tracked inputs may
        // share archive bytes while each stream retains its own current pointer.
        let content_inputs = (
            &inputs.source_id,
            &inputs.subdirectory,
            &inputs.commit,
            &inputs.dependencies,
            &inputs.lfs_objects,
            inputs.export_version,
        );
        let content = serde_json::to_vec(&content_inputs)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode artifact identity"))?;
        let id = hash(&crate::utils::frame(b"saucepan/artifact/v1", &[&content]));
        let artifact = ContentArtifact {
            id: id.clone(),
            inputs: inputs.clone(),
            archive_digest: hash(&exported.bytes),
            tree_digest: exported.tree_digest,
            archive_size: exported.bytes.len() as u64,
        };
        if policy.retain {
            archives.push((archive_path(&identity.id, &id), exported.bytes));
        }
        artifact
    };
    repository.pin(
        &format!("refs/saucepan/artifacts/{}", artifact.id),
        &inputs.commit,
    )?;
    snapshots::prepare(
        &mut source,
        &stream_id,
        descriptor,
        artifact.clone(),
        policy.retain,
    )?;
    session.trim_history(&access, &mut source)?;
    session.reserve_retired_sources(&mut access, &source)?;
    repository.verify_origin()?;
    session.commit_source(&access, source, archives)?;
    let recipe_bytes = serde_json::to_vec(recipe)
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode recipe evidence"))?;
    Ok(ArtifactHandle {
        id: artifact.id,
        inputs: artifact.inputs,
        archive_digest: artifact.archive_digest,
        tree_digest: artifact.tree_digest,
        archive_size: artifact.archive_size,
        disposition: if policy.retain {
            ArtifactDisposition::Current
        } else {
            ArtifactDisposition::Temporary
        },
        evidence: AcquisitionEvidence {
            recipe_digest: hash(&recipe_bytes),
            manifest: recipe.manifest.clone(),
            policy_revision: policy.revision,
            content_rechecked: policy.verify_content,
        },
    })
}
fn hash(bytes: &[u8]) -> String {
    crate::utils::hex(&Sha256::digest(bytes))
}
fn archive_path(source: &str, artifact: &str) -> String {
    format!("sources/{source}/archives/{artifact}.zip")
}

pub(super) fn read_archive(
    session: &Session,
    root: &Path,
    marker: Option<&Marker>,
    recipe: &Recipe,
    artifact_id: &str,
    verification: &VerificationRequest,
) -> Result<ArchiveRead> {
    let context = session.inspect_context(root, marker)?;
    preflight(recipe)?;
    let identity = sources::identify(&recipe.source, Path::new(&context.root))?;
    let mut access = session.inspect_source(
        root,
        marker,
        &identity,
        &recipe.export.subdirectory,
        verification,
    )?;
    let (record, retained) = access
        .source
        .history
        .get(artifact_id)
        .map(|h| (&h.artifact, true))
        .or_else(|| {
            access
                .source
                .current
                .values()
                .find(|c| c.artifact.id == artifact_id)
                .map(|c| (&c.artifact, c.archive_retained))
        })
        .filter(|(a, _)| a.inputs.subdirectory == recipe.export.subdirectory)
        .ok_or_else(Error::not_found)?;
    let record = record.clone();
    session.guard_dependencies(&mut access, &record.inputs)?;
    let policy = access.policy.clone();
    let repository = sources::GitRepository::open(&access.repository, &identity)?;
    // Exact recorded content is read without resolving/fetching a moving ref.
    let bytes = if retained {
        session.read_archive(&access, &record, policy.verify_content)?
    } else {
        let exported = session.reconstruct(&mut access, &repository, &record)?;
        if exported.tree_digest != record.tree_digest
            || exported.lfs_objects != record.inputs.lfs_objects
            || hash(&exported.bytes) != record.archive_digest
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "reconstructed archive does not match committed inputs",
            ));
        }
        exported.bytes
    };
    let mut source = access.source.clone();
    source.sequence = source
        .sequence
        .checked_add(1)
        .ok_or_else(|| Error::new(ErrorKind::Integrity, "source recency exhausted"))?;
    if let Some(historical) = source.history.get_mut(artifact_id) {
        historical.last_used = source.sequence;
    }
    for current in source
        .current
        .values_mut()
        .filter(|c| c.artifact.id == artifact_id)
    {
        current.last_used = source.sequence;
    }
    repository.verify_origin()?;
    session.commit_source(&access, source, vec![])?;
    Ok(ArchiveRead {
        artifact: record,
        bytes,
        content_rechecked: policy.verify_content,
    })
}
