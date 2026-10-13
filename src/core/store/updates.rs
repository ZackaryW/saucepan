//! Acquisition intent coordination and update checks. Copying remains offline.
use super::{AcquiredResource, composition, json_digest};
use crate::{
    core::{
        Store,
        retrieval::{RetrieveOptions, RetrievedResource, Retriever},
        source::AcquireOptions,
    },
    error::{Error, ErrorCode},
    models::{
        index::Record,
        policy::{Freshness, Identical, UpdatePolicy},
        retrieval::{CachePolicy, Ownership, RetrieveRequest, SourceInput},
        store::{
            AcquiredDescriptor, AcquisitionRequest, AcquisitionState, Destination, VerificationMode,
        },
    },
    utils::lock::{LockGuard, LockMode},
};
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, time::Duration};
use uuid::Uuid;

impl Store {
    /// Compose retrieval with acquisition. This owns the retrieval lifetime and
    /// can refresh without attempting to upgrade a caller-held source lock.
    pub fn acquire_composed(
        &self,
        retrieval: &RetrieveRequest,
        acquisition: &AcquisitionRequest,
    ) -> Result<AcquiredResource> {
        self.acquire_composed_with(retrieval, acquisition, &RetrieveOptions::default())
    }

    pub fn acquire_composed_with(
        &self,
        retrieval: &RetrieveRequest,
        acquisition: &AcquisitionRequest,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "use scoped application acquisition"
            )
        );
        crate::core::policy::composition::validate_composition(retrieval, acquisition, false)?;
        if !matches!(acquisition.destination, Destination::Managed {}) {
            let mut retrieval = retrieval.clone();
            let due = acquisition.update_policy.due(None, 0);
            if let SourceInput::Registered { source_uuid } = retrieval.source
                && let CachePolicy::Cached { freshness, .. } = &mut retrieval.cache
                && freshness.is_none()
            {
                *freshness = self.source(source_uuid)?.retrieval_freshness;
            }
            crate::core::policy::composition::validate_composition(&retrieval, acquisition, due)?;
            if due && let CachePolicy::Cached { freshness, .. } = &mut retrieval.cache {
                *freshness = Some(Freshness::Always {});
            }
            let retrieved = if matches!(retrieval.cache, CachePolicy::Cached { .. }) {
                self.retrieve_with(&retrieval, options)?
            } else {
                Retriever::retrieve_with(&retrieval, options)?
            };
            return composition::Acquirer::materialize_checked(&retrieved, acquisition);
        }
        let mut retrieval = retrieval.clone();
        let source_uuid = match &retrieval.source {
            SourceInput::Registered { source_uuid } => *source_uuid,
            SourceInput::Inline { source } => self.register_source(source.clone())?.uuid,
        };
        retrieval.source = SourceInput::Registered { source_uuid };
        let key = intent_key(&retrieval, acquisition)?;
        let _intent = self.acquisition_lock(&key)?;
        let previous = self.acquisition_by_key(&key)?;
        // Keep the previous selection usable through failed preparation and
        // publication. This output lease is independent of the source lease.
        let _previous_lease = match previous.as_ref() {
            Some(state) => match self.lease(state.selected, VerificationMode::Full) {
                Ok(lease) => Some(lease),
                Err(error) if Error::from_anyhow(&error).code == ErrorCode::ContentUnavailable => {
                    None
                }
                Err(error) => return Err(error),
            },
            None => None,
        };
        let now = AcquireOptions {
            test_now: options.test_now,
            ..Default::default()
        }
        .now()?;
        let due = acquisition
            .update_policy
            .due(previous.as_ref().and_then(|s| s.last_success), now);
        let source = self.source(source_uuid)?;
        if let CachePolicy::Cached { freshness, .. } = &mut retrieval.cache
            && freshness.is_none()
        {
            *freshness = source.retrieval_freshness;
        }
        crate::core::policy::composition::validate_composition(&retrieval, acquisition, due)?;
        if !due
            && !matches!(acquisition.update_policy, UpdatePolicy::Manual {})
            && acquisition.identical == Identical::Reuse
            && let Some(state) = &previous
        {
            match self.selected_acquisition(state, false) {
                Ok(result) => return Ok(result),
                Err(error) if Error::from_anyhow(&error).code == ErrorCode::ContentUnavailable => {}
                Err(error) => return Err(error),
            }
        }
        if due && let CachePolicy::Cached { freshness, .. } = &mut retrieval.cache {
            *freshness = Some(Freshness::Always {});
        }
        let retrieved = self.retrieve_with(&retrieval, options)?;
        if retrieved.descriptor.stale
            && let Some(state) = &previous
        {
            return self.selected_acquisition(state, true);
        }
        self.publish_acquisition(&retrieved, acquisition, key, previous.as_ref())
    }

    pub(crate) fn acquisition_lock(&self, key: &str) -> Result<LockGuard> {
        Ok(LockGuard::acquire(
            &self.root.join(format!(".locks/acquisition-{key}")),
            LockMode::Exclusive,
            Duration::from_secs(150),
        )?)
    }

    pub(crate) fn acquisition_by_key(&self, key: &str) -> Result<Option<AcquisitionState>> {
        Ok(self.records()?.into_iter().find_map(|record| match record {
            Record::Acquisition(state) if state.key == key => Some(*state),
            _ => None,
        }))
    }

    pub(crate) fn publish_acquisition(
        &self,
        retrieved: &RetrievedResource,
        request: &AcquisitionRequest,
        key: String,
        previous: Option<&AcquisitionState>,
    ) -> Result<AcquiredResource> {
        let legacy = composition::managed_request(retrieved, request)?;
        let prepared = composition::prepare(retrieved, request, Some(&self.root.join(".staging")))?;
        let checked = retrieved.descriptor.origin_checked && !retrieved.descriptor.stale;
        let state = AcquisitionState {
            uuid: previous.map_or_else(Uuid::new_v4, |s| s.uuid),
            key,
            revision: previous
                .map_or(Some(1), |s| s.revision.checked_add(1))
                .ok_or_else(|| anyhow::anyhow!("acquisition revision overflow"))?,
            selected: Uuid::nil(),
            request: legacy.clone(),
            acquisition: request.clone(),
            last_success: if checked {
                retrieved.descriptor.checked_at
            } else {
                previous.and_then(|s| s.last_success)
            },
            checked_source: if checked {
                Some(retrieved.descriptor.resolved.clone())
            } else {
                previous.and_then(|s| s.checked_source.clone())
            },
            verification: retrieved.descriptor.verification.clone(),
        };
        let result =
            self.publish_prepared_state(&legacy, prepared, None, true, false, Some(state.clone()))?;
        let changed =
            previous.is_none_or(|previous| previous.selected != result.acquired.entry.uuid);
        Ok(AcquiredResource {
            descriptor: AcquiredDescriptor {
                ownership: Ownership::Managed {
                    uuid: result.acquired.entry.uuid,
                },
                path: result.acquired.directory,
                selected_source: result.acquired.entry.resolved,
                checked_source: state.checked_source,
                verification: state.verification,
                reused: result.acquired.reused,
                changed,
                stale: retrieved.descriptor.stale,
                update_policy: request.update_policy,
                revision_policy: crate::core::policy::composition::effective_revision(request)?,
                reclaim: Some(result.acquired.reclaim),
            },
            _temporary: None,
            _lease: result.lease,
            _retrieval: None,
        })
    }

    fn selected_acquisition(
        &self,
        state: &AcquisitionState,
        stale: bool,
    ) -> Result<AcquiredResource> {
        let lease = self.lease(state.selected, VerificationMode::Full)?;
        let selected = self.output(state.selected)?;
        Ok(AcquiredResource {
            descriptor: AcquiredDescriptor {
                ownership: Ownership::Managed {
                    uuid: state.selected,
                },
                path: lease.directory().to_owned(),
                selected_source: selected.resolved,
                checked_source: state.checked_source.clone(),
                verification: state.verification.clone(),
                reused: true,
                changed: false,
                stale,
                update_policy: state.acquisition.update_policy,
                revision_policy: crate::core::policy::composition::effective_revision(
                    &state.acquisition,
                )?,
                reclaim: None,
            },
            _temporary: None,
            _lease: Some(lease),
            _retrieval: None,
        })
    }
}

pub(super) fn intent_key(
    retrieval: &RetrieveRequest,
    acquisition: &AcquisitionRequest,
) -> Result<String> {
    let mut acquisition = acquisition.clone();
    acquisition.revision_policy =
        crate::core::policy::composition::effective_revision(&acquisition)?;
    json_digest(&(
        &retrieval.source,
        &retrieval.selector,
        &retrieval.asset,
        retrieval.verification,
        &retrieval.expected_sha256,
        acquisition,
    ))
}

pub(super) fn selected_dependencies<'a>(
    dependencies: &'a BTreeMap<String, String>,
    folder: Option<&str>,
) -> BTreeMap<&'a str, &'a str> {
    dependencies
        .iter()
        .filter(|(key, _)| {
            let path = key.strip_prefix("lfs:").unwrap_or(key);
            folder.is_none_or(|folder| path == folder || path.starts_with(&format!("{folder}/")))
        })
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

#[cfg(test)]
mod tests;
