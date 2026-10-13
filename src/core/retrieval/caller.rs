//! Re-open explicitly caller-owned material. Serialized receipts never grant a
//! managed identity, source lease, or trusted verification evidence.
use super::{RetrievedResource, Retriever};
use crate::{
    core::source::{git, retrieve},
    error::{Error, ErrorCode},
    models::{
        retrieval::{
            CachePolicy, MaterialKind, Ownership, RetrievalPlan, RetrievedDescriptor, SourceInput,
            VerificationEvidence,
        },
        source::{Resolved, Source},
    },
    utils::{
        path::plain_metadata,
        source::git::{Git, RepositoryMaterial},
        tree,
    },
};
use anyhow::{Result, ensure};
impl Retriever {
    pub(crate) fn open_caller_planned(
        request: &RetrievalPlan,
        descriptor: &RetrievedDescriptor,
    ) -> Result<RetrievedResource> {
        crate::core::policy::composition::validate_retrieval(request)?;
        ensure!(
            matches!(descriptor.ownership, Ownership::Caller),
            Error::new(
                ErrorCode::InvalidRequest,
                "a descriptor cannot grant managed or temporary ownership"
            )
        );
        let CachePolicy::NoStore { destination } = &request.cache else {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "caller material requires its no_store request",
            )
            .into());
        };
        let SourceInput::Inline { source } = &request.source else {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "registered caller material requires metadata authorization",
            )
            .into());
        };
        ensure!(
            descriptor.source == request.source,
            Error::new(ErrorCode::InvalidRequest, "caller source identity mismatch")
        );
        ensure!(
            matches!(
                (source, &descriptor.resolved),
                (Source::Local { .. }, Resolved::Local)
                    | (Source::Url { .. }, Resolved::Url { .. })
                    | (Source::GithubRelease { .. }, Resolved::Release { .. })
                    | (
                        Source::Git { .. } | Source::GithubRepo { .. },
                        Resolved::Git { .. }
                    )
            ),
            Error::new(
                ErrorCode::InvalidRequest,
                "caller source provenance mismatch"
            )
        );
        if let (Source::Url { url }, Resolved::Url { url: resolved, .. }) =
            (source, &descriptor.resolved)
        {
            ensure!(url == resolved, "caller URL provenance mismatch");
        }
        if let Resolved::Release { assets, .. } = &descriptor.resolved {
            ensure!(
                assets.len() == 1,
                "caller release must identify one selected asset"
            );
        }
        let expected_path = match descriptor.kind {
            MaterialKind::GitRepository { .. } => destination.join("repo"),
            _ => destination.clone(),
        };
        plain_metadata(&descriptor.path)?;
        plain_metadata(&expected_path)?;
        let path = descriptor.path.canonicalize()?;
        ensure!(
            path == expected_path.canonicalize()?,
            Error::new(
                ErrorCode::InvalidRequest,
                "caller material destination mismatch"
            )
        );
        let metadata = plain_metadata(&path)?;
        let validation_source = match source {
            Source::Local { .. } => Source::Local { path: path.clone() },
            other => other.clone(),
        };
        retrieve::validate(&validation_source, request)?;
        let verification = match descriptor.kind {
            MaterialKind::GitRepository { bare } => {
                ensure!(
                    bare && metadata.is_dir(),
                    "caller Git representation must be a bare repository"
                );
                let Source::Git { origin } = git::normalize(source.clone())? else {
                    anyhow::bail!("Git source mismatch")
                };
                let Resolved::Git { commit } = &descriptor.resolved else {
                    anyhow::bail!("Git provenance missing")
                };
                let scratch = tempfile::tempdir()?;
                let inventory = Git::default().inspect_offline(
                    RepositoryMaterial {
                        origin: &origin,
                        repository: &path,
                        commit,
                        dependencies: path.parent().unwrap(),
                    },
                    scratch.path(),
                )?;
                ensure!(
                    inventory.dependencies == descriptor.dependencies,
                    Error::new(
                        ErrorCode::IntegrityError,
                        "caller dependency inventory mismatch"
                    )
                );
                VerificationEvidence {
                    git_objects_validated: true,
                    ..Default::default()
                }
            }
            MaterialKind::Directory => {
                ensure!(
                    metadata.is_dir() && matches!(source, Source::Local { .. }),
                    "directory source mismatch"
                );
                VerificationEvidence::default()
            }
            MaterialKind::File => {
                ensure!(
                    metadata.is_file()
                        && !matches!(source, Source::Git { .. } | Source::GithubRepo { .. }),
                    "file source mismatch"
                );
                ensure!(
                    metadata.len() <= tree::TreeLimits::default().bytes,
                    "caller material exceeds byte limit"
                );
                let upstream = match &descriptor.resolved {
                    Resolved::Release { assets, .. } => {
                        assets.first().and_then(|asset| asset.digest.as_deref())
                    }
                    _ => None,
                };
                let expected = retrieve::expected_digest(request, upstream)?;
                let actual = expected
                    .as_ref()
                    .map(|_| tree::digest(std::fs::File::open(&path)?))
                    .transpose()?;
                retrieve::verify_digest(expected.as_deref(), actual.as_deref())?
            }
        };
        let mut descriptor = descriptor.clone();
        descriptor.path = path;
        descriptor.verification = verification;
        descriptor.checked_at = None;
        descriptor.origin_checked = false;
        descriptor.stale = false;
        descriptor.reused = false;
        Ok(RetrievedResource {
            request: request.clone(),
            source: source.clone(),
            store: None,
            authorization: None,
            descriptor,
            _temporary: None,
            _coordination: None,
        })
    }
}
