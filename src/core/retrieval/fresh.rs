use super::RetrievedResource;
use crate::{
    core::{
        policy::composition::validate_retrieval,
        source::{AcquireOptions, retrieve},
    },
    error::{Error, ErrorCode},
    models::{
        retrieval::{
            CachePolicy, MaterialKind, Ownership, RetrieveRequest, RetrievedDescriptor, SourceInput,
        },
        source::Source,
    },
    utils::{fs::publish_prepared_directory, path::disjoint_destination, tree},
};
use anyhow::{Result, bail, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

/// Independent source retrieval. Inline fresh requests never open a Store.
pub struct Retriever;

/// Fixture seams only; cache and update policy belong to typed requests.
#[derive(Debug, Clone, Default)]
pub struct RetrieveOptions {
    pub test_now: Option<u64>,
    pub test_github_api: Option<String>,
}
impl Retriever {
    pub fn retrieve(request: &RetrieveRequest) -> Result<RetrievedResource> {
        Self::retrieve_with(request, &RetrieveOptions::default())
    }

    pub fn retrieve_with(
        request: &RetrieveRequest,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        Self::retrieve_exact(request, options, None)
    }

    pub(crate) fn retrieve_exact(
        request: &RetrieveRequest,
        options: &RetrieveOptions,
        exact: Option<&crate::models::source::Resolved>,
    ) -> Result<RetrievedResource> {
        Self::retrieve_finishing(request, options, exact, || Ok(()))
    }

    /// Hold a caller's final publication fence only after network/material work.
    pub(crate) fn retrieve_finishing<G>(
        request: &RetrieveRequest,
        options: &RetrieveOptions,
        exact: Option<&crate::models::source::Resolved>,
        finish: impl FnOnce() -> Result<G>,
    ) -> Result<RetrievedResource> {
        let options = AcquireOptions {
            test_now: options.test_now,
            test_github_api: options.test_github_api.clone(),
            ..Default::default()
        };
        validate_retrieval(request)?;
        let SourceInput::Inline { source } = &request.source else {
            bail!(Error::new(
                ErrorCode::InvalidRequest,
                "registered retrieval requires its owning store"
            ));
        };
        ensure!(
            !matches!(request.cache, CachePolicy::Cached { .. }),
            Error::new(
                ErrorCode::InvalidRequest,
                "cached retrieval requires its owning store"
            )
        );
        retrieve::validate(source, request)?;
        let destination = match &request.cache {
            CachePolicy::NoStore { destination } => Some(destination_path(source, destination)?),
            _ => None,
        };
        let work = match &destination {
            Some(path) => {
                tempfile::tempdir_in(path.parent().expect("resolved destination parent"))?
            }
            None => crate::core::consumer::temporary_workspace(None)?,
        };
        let normalized_source = crate::core::source::git::normalize(source.clone())?;
        let scratch = tempfile::tempdir()?;
        let material = retrieve::obtain(
            source,
            request,
            work.path(),
            scratch.path(),
            &options,
            exact,
        )?;
        let _finish_guard = finish()?;
        let (path, ownership, temporary) = if let Some(destination) = destination {
            let path = match material.kind {
                MaterialKind::File => {
                    // persist_noclobber supplies atomic create-new file delivery;
                    // the extra copy remains bounded by the already captured file.
                    let mut file = tempfile::NamedTempFile::new_in(destination.parent().unwrap())?;
                    crate::utils::transfer::copy_bounded(
                        fs::File::open(&material.path)?,
                        file.as_file_mut(),
                        tree::TreeLimits::default().bytes,
                        false,
                    )?;
                    file.as_file()
                        .set_permissions(fs::metadata(&material.path)?.permissions())?;
                    file.as_file().sync_all()?;
                    file.persist_noclobber(&destination)
                        .map_err(|error| error.error)?;
                    destination
                }
                MaterialKind::Directory => {
                    publish_prepared_directory(
                        &material.path,
                        &destination,
                        &delivery_lock(&destination)?,
                    )?;
                    destination
                }
                MaterialKind::GitRepository { .. } => {
                    let relative = material.path.strip_prefix(work.path())?.to_owned();
                    publish_prepared_directory(
                        work.path(),
                        &destination,
                        &delivery_lock(&destination)?,
                    )?;
                    destination.join(relative)
                }
            };
            (path, Ownership::Caller, None)
        } else {
            (material.path, Ownership::Temporary {}, Some(Rc::new(work)))
        };
        Ok(RetrievedResource {
            request: RetrieveRequest {
                source: SourceInput::Inline {
                    source: normalized_source.clone(),
                },
                ..request.clone()
            },
            source: normalized_source.clone(),
            store: None,
            authorization: None,
            descriptor: RetrievedDescriptor {
                ownership,
                path,
                kind: material.kind,
                source: SourceInput::Inline {
                    source: normalized_source,
                },
                resolved: material.resolved,
                dependencies: material.dependencies,
                verification: material.verification,
                checked_at: Some(options.now()?),
                origin_checked: true,
                reused: false,
                stale: false,
            },
            _temporary: temporary,
            _coordination: None,
        })
    }
}

fn destination_path(source: &Source, destination: &Path) -> Result<PathBuf> {
    // Resolve the parent before effects and reject capture into the local input.
    match source {
        Source::Local { path } => Ok(disjoint_destination(path, destination)?),
        Source::Git { origin } if Path::new(origin).exists() => {
            Ok(disjoint_destination(Path::new(origin), destination)?)
        }
        Source::Git { origin } if origin.starts_with("file:") => {
            let path = url::Url::parse(origin)?
                .to_file_path()
                .map_err(|_| Error::new(ErrorCode::InvalidRequest, "invalid Git file origin"))?;
            Ok(disjoint_destination(&path, destination)?)
        }
        _ => Ok(crate::utils::path::new_destination(destination)?),
    }
}

pub(crate) fn delivery_lock(destination: &Path) -> Result<PathBuf> {
    // Persistent coordination files are not payload caches. Never unlink a lock
    // while another process may hold the same native file identity.
    let locks = std::env::temp_dir().join("saucepan-delivery-locks");
    fs::create_dir_all(&locks)?;
    let digest = tree::digest(destination.to_string_lossy().to_lowercase().as_bytes())?;
    Ok(locks.join(digest))
}
