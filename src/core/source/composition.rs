//! Preserve the current managed request/result contract through the new stages.
use super::{AcquireOptions, Prepared};
use crate::{
    core::{Store, retrieval::RetrieveOptions, store::composition},
    models::{
        policy::{Freshness, PayloadVerification, RevisionPolicy},
        retrieval::{AssetSelection, CachePolicy, RetrieveRequest, SourceInput},
        source::Source,
        store::{AcquisitionRequest, Format, Request},
    },
};
use anyhow::{Result, ensure};

impl Store {
    /// Exact recovery fetches saved provenance into private scratch. It must not
    /// replace a moving selector's cache or advance its freshness proof.
    pub(crate) fn prepare_exact_download(
        &self,
        request: &Request,
        exact: &crate::models::source::Resolved,
        options: &AcquireOptions,
    ) -> Result<Prepared> {
        let source = self.source(request.source_uuid)?;
        let retrieval = RetrieveRequest {
            source: SourceInput::Inline {
                source: source.source,
            },
            selector: request.selector.clone(),
            asset: AssetSelection {
                platform: request.selection.platform.clone(),
                filters: request.selection.filters.clone(),
                selection_policy: request.selection.selection_policy.clone(),
            },
            cache: CachePolicy::Temporary {},
            verification: PayloadVerification::IfAvailable,
            expected_sha256: request.selection.sha256.clone(),
        };
        let retrieved = crate::core::retrieval::Retriever::retrieve_exact(
            &retrieval,
            &RetrieveOptions {
                test_now: options.test_now,
                test_github_api: options.test_github_api.clone(),
            },
            Some(exact),
        )?;
        composition::prepare(
            &retrieved,
            &AcquisitionRequest {
                format: request.selection.format,
                filename: request.selection.filename.clone(),
                folder: request.selection.folder.clone(),
                bin: request.selection.bin.clone(),
                ..Default::default()
            },
            Some(&self.root.join(".staging")),
        )
    }

    pub(crate) fn prepare_composed(
        &self,
        request: &Request,
        options: &AcquireOptions,
    ) -> Result<Prepared> {
        let (retrieval, acquired) = self.composition_requests(request, options)?;
        let retrieved = self.retrieve_prevalidated(
            &retrieval,
            &RetrieveOptions {
                test_now: options.test_now,
                test_github_api: options.test_github_api.clone(),
            },
        )?;
        composition::prepare(&retrieved, &acquired, Some(&self.root.join(".staging")))
    }

    pub(crate) fn composition_requests(
        &self,
        request: &Request,
        options: &AcquireOptions,
    ) -> Result<(RetrieveRequest, AcquisitionRequest)> {
        let source = self.source(request.source_uuid)?;
        match &source.source {
            Source::Local { .. } | Source::Git { .. } | Source::GithubRepo { .. } => ensure!(
                request.selection.format == Format::Natural,
                "local/Git sources require natural format"
            ),
            Source::Url { .. } | Source::GithubRelease { .. } => ensure!(
                matches!(request.selection.format, Format::Raw | Format::Zip),
                "downloads require raw or zip format"
            ),
        }
        let retrieval = RetrieveRequest {
            source: SourceInput::Registered {
                source_uuid: request.source_uuid,
            },
            selector: request.selector.clone(),
            asset: AssetSelection {
                platform: request.selection.platform.clone(),
                filters: request.selection.filters.clone(),
                selection_policy: request.selection.selection_policy.clone(),
            },
            cache: CachePolicy::Cached {
                freshness: options.force_refresh.then_some(Freshness::Always {}),
                allow_stale: options.allow_stale,
            },
            verification: PayloadVerification::IfAvailable,
            expected_sha256: request.selection.sha256.clone(),
        };
        let acquired = AcquisitionRequest {
            format: request.selection.format,
            filename: if matches!(source.source, Source::Local { .. }) {
                None
            } else {
                request.selection.filename.clone()
            },
            folder: request.selection.folder.clone(),
            bin: request.selection.bin.clone(),
            identical: request.policies.identical,
            revision_policy: Some(RevisionPolicy::try_from(&request.policies.eviction)?),
            protection: request.policies.protection.clone(),
            ..Default::default()
        };
        crate::core::policy::composition::validate_composition(&retrieval, &acquired, false)?;
        Ok((retrieval, acquired))
    }
}
