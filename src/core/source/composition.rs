//! Execute managed output plans through source retrieval and offline copying.
use super::{AcquireOptions, Prepared};
use crate::{
    core::{Store, retrieval::RetrieveOptions, store::composition},
    models::{
        policy::PayloadVerification,
        retrieval::{AssetSelection, CachePolicy, RetrievalPlan, SourceInput},
        source::Source,
        store::{AcquisitionPlan, Format, OutputPlan},
    },
};
use anyhow::{Result, ensure};

impl Store {
    /// Exact recovery fetches saved provenance into private scratch. It must not
    /// replace a moving selector's cache or advance its freshness proof.
    pub(crate) fn prepare_exact_download(
        &self,
        request: &OutputPlan,
        exact: &crate::models::source::Resolved,
        options: &AcquireOptions,
    ) -> Result<Prepared> {
        let source = self.source(request.source_uuid)?;
        let mut retrieval = RetrievalPlan {
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
        {
            let adopted = request.adopted.as_deref().ok_or_else(|| {
                crate::error::Error::new(
                    crate::error::ErrorCode::IntegrityError,
                    "output is missing adopted policy evidence",
                )
            })?;
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
            &AcquisitionPlan {
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
        request: &OutputPlan,
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
        request: &OutputPlan,
        _options: &AcquireOptions,
    ) -> Result<(RetrievalPlan, AcquisitionPlan)> {
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
        let mut retrieval = RetrievalPlan {
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
                freshness: None,
                allow_stale: false,
            },
            verification: PayloadVerification::IfAvailable,
            expected_sha256: request.selection.sha256.clone(),
        };
        let mut acquired = AcquisitionPlan {
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
            revision_policy: Some(request.policies.revision),
            protection: request.policies.protection.clone(),
            ..Default::default()
        };
        {
            let adopted = request.adopted.as_deref().ok_or_else(|| {
                crate::error::Error::new(
                    crate::error::ErrorCode::IntegrityError,
                    "output is missing adopted policy evidence",
                )
            })?;
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
            retrieval.cache = CachePolicy::Cached {
                freshness: Some(behavior.freshness),
                allow_stale: behavior.allow_stale,
            };
            retrieval.verification = behavior.payload_verification;
            retrieval.provenance = adopted.evidence.values().cloned().collect();
            acquired.provenance = retrieval.provenance.clone();
            acquired.evaluated = adopted.pinned();
            acquired.update_policy = behavior.update;
            acquired.revision_policy = Some(behavior.revision);
        }
        crate::core::policy::composition::validate_composition(&retrieval, &acquired, false)?;
        Ok((retrieval, acquired))
    }
}
