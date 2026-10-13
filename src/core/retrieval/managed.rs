//! Managed retrieval uses the existing catalog and journal, never output records.
use super::{RetrieveOptions, RetrievedResource};
use crate::{
    core::{
        Store,
        index::Journal,
        source::{AcquireOptions, remote, retrieve},
    },
    error::{Error, ErrorCode},
    models::{
        index::Record,
        policy::Freshness,
        retrieval::{
            CachePolicy, MaterialKind, RetrievalRecord, RetrievalState, RetrieveRequest,
            SourceInput,
        },
        source::{Discovery, Resolved, Source, SourceRecord},
    },
    utils::{
        lock::{LockGuard, LockMode},
        path::beneath,
    },
};
use anyhow::{Context, Result, bail, ensure};
use std::{fs, time::Duration};
use uuid::Uuid;

impl Store {
    pub fn retrieve(&self, request: &RetrieveRequest) -> Result<RetrievedResource> {
        self.retrieve_with(request, &RetrieveOptions::default())
    }

    pub fn retrieve_with(
        &self,
        request: &RetrieveRequest,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "use the scoped application retrieval interface"
            )
        );
        self.retrieve_prevalidated(request, options)
    }

    /// Internal composition after the owning operation authorizes app/source/scope.
    /// App identities are retained; this must never become an administrative fallback.
    pub(crate) fn retrieve_prevalidated(
        &self,
        request: &RetrieveRequest,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        ensure!(
            !self.metadata_only,
            Error::new(
                ErrorCode::InvalidRequest,
                "metadata-only access cannot retrieve cached material"
            )
        );
        crate::core::policy::composition::validate_retrieval(request)?;
        let CachePolicy::Cached {
            freshness,
            allow_stale,
        } = request.cache
        else {
            bail!(Error::new(
                ErrorCode::InvalidRequest,
                "fresh retrieval uses its independent ownership interface"
            ));
        };
        let source = match &request.source {
            SourceInput::Registered { source_uuid } => self.source(*source_uuid)?,
            SourceInput::Inline { source } => {
                ensure!(
                    self.session.is_none(),
                    Error::new(
                        ErrorCode::AccessDenied,
                        "application retrieval requires a registered source"
                    )
                );
                retrieve::validate(source, request)?;
                self.register_source(source.clone())?
            }
        };
        retrieve::validate(&source.source, request)?;
        let options = AcquireOptions {
            test_now: options.test_now,
            test_github_api: options.test_github_api.clone(),
            ..Default::default()
        };
        let now = options.now()?;
        let lock_path = self.root.join(format!(".locks/source-{}", source.uuid));
        let guard = LockGuard::acquire(&lock_path, LockMode::Shared, Duration::from_secs(150))?;
        let source = self.source(source.uuid)?;
        let policy = freshness
            .or(source.retrieval_freshness)
            .unwrap_or(Freshness::MaxAge {
                seconds: source.refresh.ttl_seconds,
            });
        let previous = self.matching_retrieval(&source, request)?;
        if let Some(record) = &previous
            && usable_age(policy, record.checked_at, now)
            && (!matches!(source.source, Source::Local { .. })
                || matches!(policy, Freshness::CacheOnly {}))
            && self.cache_hit(record, request)?
        {
            return self.retrieved_handle(record, request, guard, true, false, false);
        }
        ensure!(
            !matches!(policy, Freshness::CacheOnly {}),
            Error::new(
                ErrorCode::ContentUnavailable,
                "retrieval cache has no usable material"
            )
        );
        drop(guard);
        let guard = LockGuard::acquire(&lock_path, LockMode::Exclusive, Duration::from_secs(150))?;
        let mut source = self.source(source.uuid)?;
        let policy = freshness
            .or(source.retrieval_freshness)
            .unwrap_or(Freshness::MaxAge {
                seconds: source.refresh.ttl_seconds,
            });
        let previous = self.matching_retrieval(&source, request)?;
        // A concurrent max-age retrieval may have satisfied the request while we waited.
        if let Some(record) = &previous
            && !matches!(policy, Freshness::Always {})
            && usable_age(policy, record.checked_at, now)
            && !matches!(source.source, Source::Local { .. })
            && self.cache_hit(record, request)?
        {
            drop(guard);
            let guard = LockGuard::acquire(&lock_path, LockMode::Shared, Duration::from_secs(150))?;
            let record = self.retrieval_record(record.uuid)?;
            self.verify_retrieval(&record, request)?;
            return self.retrieved_handle(&record, request, guard, true, false, false);
        }
        ensure!(
            !matches!(policy, Freshness::CacheOnly {}),
            Error::new(
                ErrorCode::ContentUnavailable,
                "retrieval cache has no usable material"
            )
        );
        let fetched =
            self.fetch_retrieval(&mut source, request, &options, policy, previous.as_ref());
        let (record, reused, stale, origin_checked) = match fetched {
            Ok((record, reused, origin_checked)) => (record, reused, false, origin_checked),
            Err(error) if allow_stale => {
                let record = previous.context(error)?;
                self.verify_retrieval(&record, request)?;
                (record, true, true, false)
            }
            Err(error) => return Err(error),
        };
        drop(guard);
        let guard = LockGuard::acquire(&lock_path, LockMode::Shared, Duration::from_secs(150))?;
        // Purge could have acquired the source lock in the handover interval.
        let record = self.retrieval_record(record.uuid)?;
        self.verify_retrieval(&record, request)?;
        self.retrieved_handle(&record, request, guard, reused, stale, origin_checked)
    }

    fn matching_retrieval(
        &self,
        source: &SourceRecord,
        request: &RetrieveRequest,
    ) -> Result<Option<RetrievalRecord>> {
        Ok(self
            .records()?
            .into_iter()
            .filter_map(|record| match record {
                Record::Retrieval(record)
                    if record.source_uuid == source.uuid
                        && record.state == RetrievalState::Ready
                        && record.request.selector == request.selector
                        && record.request.asset == request.asset =>
                {
                    Some(*record)
                }
                _ => None,
            })
            .max_by_key(|record| (record.checked_at, record.uuid)))
    }

    fn fetch_retrieval(
        &self,
        source: &mut SourceRecord,
        request: &RetrieveRequest,
        options: &AcquireOptions,
        policy: Freshness,
        previous: Option<&RetrievalRecord>,
    ) -> Result<(RetrievalRecord, bool, bool)> {
        ensure!(
            !crate::core::consumer::has_source_consumers(self, source.uuid)?,
            Error::new(
                ErrorCode::ContentUnavailable,
                "source is in use by a child consumer"
            )
        );
        let stage = tempfile::tempdir_in(self.root.join(".staging"))?;
        let content = stage.path().join("content");
        fs::create_dir(&content)?;
        let scratch = tempfile::tempdir()?;
        let source_work = beneath(&self.root, &format!("source/{}", source.uuid))?;
        fs::create_dir_all(&source_work)?;
        let is_git = matches!(
            source.source,
            Source::Git { .. } | Source::GithubRepo { .. }
        );
        let discovery_key = serde_json::to_string(&request.selector)?;
        let now = options.now()?;
        let cached_discovery = source
            .discovery
            .get(&discovery_key)
            .filter(|entry| usable_age(policy, entry.checked_at, now));
        let checked_at = if is_git || matches!(source.source, Source::Local { .. }) {
            now
        } else {
            cached_discovery.map_or(now, |entry| entry.checked_at)
        };
        let origin_checked =
            is_git || matches!(source.source, Source::Local { .. }) || cached_discovery.is_none();
        let discovered = if is_git || matches!(source.source, Source::Local { .. }) {
            None
        } else if let Some(discovered) = cached_discovery {
            Some(discovered.resolved.clone())
        } else {
            Some(remote::discover(
                &source.source,
                &request.selector,
                options,
            )?)
        };
        // Metadata freshness and material publication are separate. A successful
        // discovery is shared across asset selections even if this download fails.
        if let Some(resolved) = &discovered {
            source.discovery.insert(
                discovery_key.clone(),
                Discovery {
                    checked_at,
                    resolved: resolved.clone(),
                },
            );
            let _lock = self.lock()?;
            self.save_source(source)?;
        }
        if let (Some(old), Some(discovered)) = (previous, discovered.as_ref()) {
            let selected = selected_provenance(discovered, request)?;
            let reusable = match &selected {
                Resolved::Url {
                    etag,
                    last_modified,
                    ..
                } => !origin_checked || etag.is_some() || last_modified.is_some(),
                _ => true,
            };
            if reusable && old.resolved == selected && self.cache_hit(old, request)? {
                let mut record = old.clone();
                record.checked_at = checked_at;
                let _lock = self.lock()?;
                self.commit(Journal {
                    records: vec![Record::Retrieval(Box::new(record.clone()))],
                    ..Default::default()
                })?;
                return Ok((record, true, origin_checked));
            }
        }
        let material = retrieve::obtain(
            &source.source,
            request,
            if is_git { &source_work } else { &content },
            scratch.path(),
            options,
            discovered.as_ref(),
        )?;
        let files = if is_git {
            Default::default()
        } else {
            crate::core::store::manifest(&content)?
        };
        let mut verification = material.verification;
        if matches!(material.kind, MaterialKind::File) {
            verification.content_sha256 =
                files.get("material").and_then(|entry| entry.digest.clone());
        }
        let reused = previous.is_some_and(|old| {
            old.resolved == material.resolved
                && old.kind == material.kind
                && old.dependencies == material.dependencies
                && old.files == files
        }) && match previous {
            Some(old) => self.cache_hit(old, request)?,
            None => false,
        };
        let record = RetrievalRecord {
            uuid: if reused {
                previous.unwrap().uuid
            } else {
                Uuid::new_v4()
            },
            source_uuid: source.uuid,
            request: RetrieveRequest {
                source: SourceInput::Registered {
                    source_uuid: source.uuid,
                },
                ..request.clone()
            },
            kind: material.kind,
            resolved: material.resolved,
            dependencies: material.dependencies,
            verification,
            files,
            checked_at,
            state: RetrievalState::Ready,
        };
        let _lock = self.lock()?;
        let stage_name = stage
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .context("non-UTF8 stage")?
            .to_owned();
        self.commit(Journal {
            records: vec![Record::Retrieval(Box::new(record.clone()))],
            retrieval_stage: (!is_git && !reused).then_some((stage_name, record.uuid)),
            ..Default::default()
        })?;
        source.discovery.insert(
            discovery_key,
            Discovery {
                checked_at,
                resolved: discovered.unwrap_or_else(|| record.resolved.clone()),
            },
        );
        self.save_source(source)?;
        Ok((record, reused, origin_checked))
    }
}

fn usable_age(policy: Freshness, checked: u64, now: u64) -> bool {
    match policy {
        Freshness::CacheOnly {} => true,
        Freshness::Always {} => false,
        Freshness::MaxAge { seconds } => now.checked_sub(checked).is_some_and(|age| age < seconds),
    }
}

fn selected_provenance(resolved: &Resolved, request: &RetrieveRequest) -> Result<Resolved> {
    match resolved {
        Resolved::Release { id, tag, assets } => {
            let selection = crate::models::store::Selection {
                platform: request.asset.platform.clone(),
                filters: request.asset.filters.clone(),
                selection_policy: request.asset.selection_policy.clone(),
                ..Default::default()
            };
            Ok(Resolved::Release {
                id: *id,
                tag: tag.clone(),
                assets: vec![crate::core::policy::select_asset(assets, &selection)?.clone()],
            })
        }
        other => Ok(other.clone()),
    }
}
