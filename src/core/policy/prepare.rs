//! Pure translation from references into provider/copy execution settings.
//! No filesystem, registration, or authentication is performed here.
use super::{
    catalog::PolicyCatalog,
    resolve::{self, Inputs, Operation},
};
use crate::{
    error::{Error, ErrorCode},
    models::{
        policy::{
            PolicyRefs, Protection,
            effective::EvaluatedPolicies,
            record::CacheMode,
            requests::{Acquisition, Retrieval},
        },
        retrieval::{AssetSelection, CachePolicy, RetrieveRequest},
        store::{AcquisitionRequest, Destination},
    },
};
use anyhow::{Result, ensure};

pub struct Attachments<'a> {
    pub source: &'a PolicyRefs,
    pub application: &'a PolicyRefs,
}

pub struct PreparedRetrieval {
    pub(crate) request: RetrieveRequest,
    pub policies: EvaluatedPolicies,
}
pub struct PreparedAcquisition {
    pub(crate) request: AcquisitionRequest,
    pub policies: EvaluatedPolicies,
}

pub fn retrieval(
    catalog: &PolicyCatalog,
    attachments: &Attachments<'_>,
    input: &Retrieval,
) -> Result<PreparedRetrieval> {
    let policies = resolve::evaluate(
        catalog,
        Inputs {
            source: attachments.source,
            application: attachments.application,
            request: &input.policy_refs,
            pinned: None,
            operation: Operation::Retrieve,
            managed_output: false,
            update_due: false,
        },
    )?;
    let b = &policies.behavior;
    let cache = match b.cache {
        CacheMode::Cached => CachePolicy::Cached {
            freshness: Some(b.freshness),
            allow_stale: b.allow_stale,
        },
        CacheMode::Temporary => CachePolicy::Temporary {},
        CacheMode::NoStore => CachePolicy::NoStore {
            destination: input.destination.clone().ok_or_else(|| {
                Error::new(ErrorCode::InvalidRequest, "no_store requires a destination")
            })?,
        },
    };
    ensure!(
        b.cache == CacheMode::NoStore || input.destination.is_none(),
        Error::new(
            ErrorCode::InvalidRequest,
            "retrieval destination requires no_store policy"
        )
    );
    let request = RetrieveRequest {
        provenance: policies
            .evidence
            .values()
            .cloned()
            .chain(
                input
                    .asset
                    .filter_refs
                    .ids()
                    .iter()
                    .map(|id| resolve::evidence(catalog.get(*id)?))
                    .collect::<crate::error::Result<Vec<_>>>()?,
            )
            .collect(),
        source: input.source.clone(),
        selector: input.selector.clone(),
        asset: AssetSelection {
            platform: input.asset.platform.clone(),
            filters: resolve::filters(catalog, &input.asset.filter_refs)?,
            selection_policy: b.selection.clone(),
        },
        cache,
        verification: b.payload_verification,
        expected_sha256: input.expected_sha256.clone(),
    };
    super::composition::validate_retrieval(&request)?;
    Ok(PreparedRetrieval { request, policies })
}

pub fn acquisition(
    catalog: &PolicyCatalog,
    attachments: &Attachments<'_>,
    input: &Acquisition,
) -> Result<PreparedAcquisition> {
    let managed = matches!(input.destination, Destination::Managed {});
    let policies = resolve::evaluate(
        catalog,
        Inputs {
            source: attachments.source,
            application: attachments.application,
            request: &input.policy_refs,
            pinned: None,
            operation: Operation::Acquire,
            managed_output: managed,
            update_due: false,
        },
    )?;
    let b = &policies.behavior;
    let request = AcquisitionRequest {
        provenance: policies.evidence.values().cloned().collect(),
        destination: input.destination.clone(),
        format: input.format,
        filename: input.filename.clone(),
        folder: input.folder.clone(),
        bin: input.bin.clone(),
        identical: b.identical,
        update_policy: b.update,
        revision_policy: managed.then_some(b.revision),
        protection: b.useable_app.then_some(Protection::UseableApp),
    };
    Ok(PreparedAcquisition { request, policies })
}

/// Every definition used by durable records must already be registered. A
/// portable catalog is never implicitly published as a side effect of fetching.
pub fn require_registered(
    registered: &PolicyCatalog,
    combined: &PolicyCatalog,
    references: impl IntoIterator<Item = uuid::Uuid>,
) -> Result<()> {
    for id in references {
        let expected = combined.get(id)?;
        let actual = registered.get(id).map_err(|_| {
            Error::new(
                ErrorCode::InvalidRequest,
                format!("policy {id} must be registered before managed retrieval or acquisition"),
            )
        })?;
        ensure!(
            actual == expected,
            Error::new(ErrorCode::PolicyConflict, "policy catalog shadowing")
        );
    }
    Ok(())
}
