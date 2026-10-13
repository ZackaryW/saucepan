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
        let mut retrieval = RetrieveRequest {
            provenance: Vec::new(),
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
        if let Some(adopted) = &request.adopted {
            retrieval.verification = adopted.behavior.payload_verification;
            retrieval.provenance = adopted.evidence.values().cloned().collect();
        }
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
        let mut retrieval = RetrieveRequest {
            provenance: Vec::new(),
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
        let mut acquired = AcquisitionRequest {
            adopted: request.adopted.clone(),
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
        if let Some(adopted) = &request.adopted {
            let snapshot = self.metadata.snapshot()?;
            let catalog = crate::core::policy::persistence::load(&self.metadata, &snapshot)?;
            crate::core::policy::claims::validate_adopted(&catalog, adopted)?;
            let behavior = &adopted.behavior;
            ensure!(
                behavior.cache == crate::models::policy::record::CacheMode::Cached,
                crate::error::Error::new(
                    crate::error::ErrorCode::PolicyConflict,
                    "managed claims require cached retrieval"
                )
            );
            ensure!(
                !options.force_refresh || !matches!(behavior.freshness, Freshness::CacheOnly {}),
                crate::error::Error::new(
                    crate::error::ErrorCode::PolicyConflict,
                    "forced update conflicts with cache_only"
                )
            );
            retrieval.cache = CachePolicy::Cached {
                freshness: Some(if options.force_refresh {
                    Freshness::Always {}
                } else {
                    behavior.freshness
                }),
                allow_stale: behavior.allow_stale,
            };
            retrieval.verification = behavior.payload_verification;
            retrieval.provenance = adopted.evidence.values().cloned().collect();
            acquired.provenance = retrieval.provenance.clone();
            acquired.update_policy = behavior.update;
            acquired.revision_policy = Some(behavior.revision);
        }
        crate::core::policy::composition::validate_composition(&retrieval, &acquired, false)?;
        Ok((retrieval, acquired))
    }
}
