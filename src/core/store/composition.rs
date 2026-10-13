use super::access::ResourceLease;
use crate::{core::retrieval::RetrievedResource, models::store::AcquiredDescriptor};
use std::{path::Path, rc::Rc};

use crate::{
    core::{
        policy::composition::{effective_revision, validate_composition},
        source::Prepared,
    },
    error::{Error, ErrorCode},
    models::{
        policy::{Policies, UpdatePolicy},
        retrieval::{Ownership, SourceInput},
        store::{AcquisitionRequest, Destination, OutputPlan, Selection},
    },
    utils::{fs::publish_prepared_directory, path::disjoint_destination},
};
use anyhow::{Result, ensure};

pub struct Acquirer;
impl Acquirer {
    /// Root-free fresh retrieval followed by independent materialization.
    pub fn retrieve_and_acquire(
        retrieval: &crate::models::retrieval::RetrieveRequest,
        request: &AcquisitionRequest,
    ) -> Result<AcquiredResource> {
        Self::retrieve_and_acquire_with(retrieval, request, &Default::default())
    }
    pub fn retrieve_and_acquire_with(
        retrieval: &crate::models::retrieval::RetrieveRequest,
        request: &AcquisitionRequest,
        options: &crate::core::retrieval::RetrieveOptions,
    ) -> Result<AcquiredResource> {
        validate_composition(retrieval, request, request.update_policy.due(None, 0))?;
        preflight_destination(retrieval, request)?;
        let retrieved = crate::core::retrieval::Retriever::retrieve_with(retrieval, options)?;
        Self::materialize_checked(&retrieved, request)
    }

    /// Release this source lease before orchestrating an update. The caller may
    /// retain prior acquired-output leases independently.
    pub fn refresh(
        retrieved: RetrievedResource,
        request: &AcquisitionRequest,
    ) -> Result<AcquiredResource> {
        Self::refresh_with(retrieved, request, &Default::default())
    }

    pub fn refresh_with(
        retrieved: RetrievedResource,
        request: &AcquisitionRequest,
        options: &crate::core::retrieval::RetrieveOptions,
    ) -> Result<AcquiredResource> {
        let retrieval = retrieved.request.clone();
        let store = retrieved.store.clone();
        let authorization = retrieved.authorization.clone();
        drop(retrieved);
        match store {
            Some(store) => {
                store.acquire_composed_authorized(&retrieval, request, options, authorization)
            }
            None => Self::retrieve_and_acquire_with(&retrieval, request, options),
        }
    }

    pub(crate) fn materialize_checked(
        retrieved: &RetrievedResource,
        request: &AcquisitionRequest,
    ) -> Result<AcquiredResource> {
        let mut offline = request.clone();
        offline.update_policy = UpdatePolicy::Manual {};
        let mut result = Self::acquire(retrieved, &offline)?;
        result.descriptor.update_policy = request.update_policy;
        result.descriptor.checked_source =
            (!retrieved.descriptor.stale).then(|| retrieved.descriptor.resolved.clone());
        Ok(result)
    }
    /// Materialize a retrieval without fetching. Update orchestration uses the
    /// owning acquisition intent; callers may reuse this input for several copies.
    pub fn acquire(
        retrieved: &RetrievedResource,
        request: &AcquisitionRequest,
    ) -> Result<AcquiredResource> {
        validate_composition(&retrieved.request, request, false)?;
        if let (Some(store), Some(authorization)) = (&retrieved.store, &retrieved.authorization) {
            let SourceInput::Registered { source_uuid } = retrieved.request.source else {
                return Err(
                    Error::new(ErrorCode::AccessDenied, "registered source required").into(),
                );
            };
            authorization.check(store, source_uuid, Some(request))?;
        }
        ensure!(
            matches!(request.update_policy, UpdatePolicy::Manual {}),
            Error::new(
                ErrorCode::InvalidRequest,
                "automatic updates require acquisition orchestration"
            )
        );
        if matches!(request.destination, Destination::Managed {}) {
            let store = retrieved.store.as_ref().ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidRequest,
                    "managed output requires its retrieval store",
                )
            })?;
            ensure!(
                store.session.is_none() || retrieved.authorization.is_some(),
                Error::new(
                    ErrorCode::AccessDenied,
                    "use scoped application acquisition"
                )
            );
            let key = super::updates::scoped_intent_key(
                &retrieved.request,
                request,
                retrieved.authorization.as_ref(),
            )?;
            // A retained source lease cannot wait for an intent whose owner may
            // itself be waiting to refresh that source. Fail immediately instead.
            let _intent = crate::utils::lock::LockGuard::acquire(
                &store.root.join(format!(".locks/acquisition-{key}")),
                crate::utils::lock::LockMode::Exclusive,
                std::time::Duration::ZERO,
            )?;
            let previous = store.acquisition_by_key(&key)?;
            return store.publish_acquisition(retrieved, request, key, previous.as_ref(), None);
        }
        let destination = match &request.destination {
            Destination::Directory { path } => {
                let protected = retrieved
                    .store
                    .as_ref()
                    .map(|store| store.root())
                    .unwrap_or_else(|| {
                        if matches!(
                            retrieved.descriptor.kind,
                            crate::models::retrieval::MaterialKind::GitRepository { .. }
                        ) {
                            retrieved.source_path().parent().unwrap()
                        } else {
                            retrieved.source_path()
                        }
                    });
                Some(disjoint_destination(protected, path)?)
            }
            _ => None,
        };
        let prepared = prepare(
            retrieved,
            request,
            destination.as_ref().and_then(|path| path.parent()),
        )?;
        // Fence authorization changes while copying before caller-visible delivery.
        let _authorization_lock = retrieved
            .store
            .as_ref()
            .map(|store| store.lock())
            .transpose()?;
        if let Some(store) = &retrieved.store {
            store.check_policy_fence()?;
        }
        if let (Some(store), Some(authorization)) = (&retrieved.store, &retrieved.authorization)
            && let SourceInput::Registered { source_uuid } = retrieved.request.source
        {
            authorization.check(store, source_uuid, Some(request))?;
        }
        let (path, ownership, temporary) = match destination {
            Some(destination) => {
                publish_prepared_directory(
                    &prepared.temp.path().join("content"),
                    &destination,
                    &crate::core::retrieval::delivery_lock(&destination)?,
                )?;
                (destination, Ownership::Caller, None)
            }
            None => (
                prepared.temp.path().join("content"),
                Ownership::Temporary {},
                Some(Rc::new(prepared.temp)),
            ),
        };
        Ok(AcquiredResource {
            descriptor: AcquiredDescriptor {
                ownership,
                path,
                selected_source: retrieved.descriptor.resolved.clone(),
                checked_source: None,
                verification: retrieved.descriptor.verification.clone(),
                reused: false,
                changed: true,
                stale: retrieved.descriptor.stale,
                update_policy: request.update_policy,
                revision_policy: None,
                reclaim: None,
            },
            _temporary: temporary,
            _lease: None,
            _retrieval: None,
        })
    }
}

pub(crate) fn preflight_destination(
    retrieval: &crate::models::retrieval::RetrieveRequest,
    request: &AcquisitionRequest,
) -> Result<()> {
    if let Destination::Directory { path } = &request.destination {
        let output = crate::utils::path::new_destination(path)?;
        if let crate::models::retrieval::CachePolicy::NoStore { destination } = &retrieval.cache {
            let source = crate::utils::path::new_destination(destination)?;
            ensure!(
                !source.starts_with(&output) && !output.starts_with(&source),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "source and output destinations overlap"
                )
            );
        }
    }
    Ok(())
}

pub(crate) fn prepare(
    retrieved: &RetrievedResource,
    request: &AcquisitionRequest,
    staging: Option<&Path>,
) -> Result<Prepared> {
    let temp = match staging {
        Some(path) => tempfile::tempdir_in(path)?,
        None => crate::core::consumer::temporary_workspace(None)?,
    };
    super::materialize::copy(retrieved, request, temp.path())?;
    Ok(Prepared {
        temp,
        resolved: retrieved.descriptor.resolved.clone(),
        dependencies: retrieved.descriptor.dependencies.clone(),
        update_checked: retrieved.descriptor.origin_checked
            && !matches!(
                retrieved.source,
                crate::models::source::Source::Local { .. }
            ),
        stale: retrieved.descriptor.stale,
    })
}

pub(crate) fn managed_request(
    retrieved: &RetrievedResource,
    request: &AcquisitionRequest,
) -> Result<OutputPlan> {
    let SourceInput::Registered { source_uuid } = retrieved.descriptor.source else {
        return Err(
            Error::new(ErrorCode::InvalidRequest, "managed source identity missing").into(),
        );
    };
    let revision = effective_revision(request)?.expect("managed revision policy");
    Ok(OutputPlan {
        adopted: request.adopted.clone(),
        source_uuid,
        selector: retrieved.request.selector.clone(),
        selection: Selection {
            platform: retrieved.request.asset.platform.clone(),
            filters: retrieved.request.asset.filters.clone(),
            selection_policy: retrieved.request.asset.selection_policy.clone(),
            format: request.format,
            filename: request.filename.clone(),
            folder: request.folder.clone(),
            bin: request.bin.clone(),
            sha256: retrieved.request.expected_sha256.clone(),
        },
        policies: Policies {
            identical: request.identical,
            revision,
            protection: request.protection.clone(),
        },
    })
}

/// Copied files with explicit ownership. A native managed lease or temporary
/// owner stays alive with the result; descriptors alone do not own either.
pub struct AcquiredResource {
    pub(crate) descriptor: AcquiredDescriptor,
    pub(crate) _temporary: Option<Rc<tempfile::TempDir>>,
    pub(crate) _lease: Option<ResourceLease>,
    pub(crate) _retrieval: Option<RetrievedResource>,
}
impl AcquiredResource {
    pub fn path(&self) -> &Path {
        &self.descriptor.path
    }

    pub fn descriptor(&self) -> &AcquiredDescriptor {
        &self.descriptor
    }
}
