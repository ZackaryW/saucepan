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
        store::{AcquisitionRequest, Destination, Request, Selection},
    },
    utils::{fs::publish_prepared_directory, path::disjoint_destination},
};
use anyhow::{Result, ensure};

pub struct Acquirer;
impl Acquirer {
    /// Materialize a retrieval without fetching. Update orchestration uses the
    /// owning acquisition intent; callers may reuse this input for several copies.
    pub fn acquire(
        retrieved: &RetrievedResource,
        request: &AcquisitionRequest,
    ) -> Result<AcquiredResource> {
        validate_composition(&retrieved.request, request, false)?;
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
            let legacy = managed_request(retrieved, request)?;
            let prepared = prepare(retrieved, request, Some(&store.root.join(".staging")))?;
            let result = store.publish_prepared(&legacy, prepared, None, true, false)?;
            return Ok(AcquiredResource {
                descriptor: AcquiredDescriptor {
                    ownership: Ownership::Managed {
                        uuid: result.acquired.entry.uuid,
                    },
                    path: result.acquired.directory,
                    selected_source: result.acquired.entry.resolved,
                    checked_source: Some(retrieved.descriptor.resolved.clone()),
                    verification: retrieved.descriptor.verification.clone(),
                    reused: result.acquired.reused,
                    changed: !result.acquired.reused,
                    stale: retrieved.descriptor.stale,
                    update_policy: request.update_policy,
                    revision_policy: effective_revision(request)?,
                    reclaim: Some(result.acquired.reclaim),
                },
                _temporary: None,
                _lease: result.lease,
                _retrieval: None,
            });
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

pub(crate) fn prepare(
    retrieved: &RetrievedResource,
    request: &AcquisitionRequest,
    staging: Option<&Path>,
) -> Result<Prepared> {
    let temp = match staging {
        Some(path) => tempfile::tempdir_in(path)?,
        None => tempfile::tempdir()?,
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

fn managed_request(retrieved: &RetrievedResource, request: &AcquisitionRequest) -> Result<Request> {
    let SourceInput::Registered { source_uuid } = retrieved.descriptor.source else {
        return Err(
            Error::new(ErrorCode::InvalidRequest, "managed source identity missing").into(),
        );
    };
    let revision = effective_revision(request)?.expect("managed revision policy");
    Ok(Request {
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
            eviction: revision.try_into()?,
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
