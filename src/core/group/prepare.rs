use super::{
    plan::{Plan, Work},
    *,
};
use crate::{
    core::{
        lease::GroupLease,
        retrieval::RetrieveOptions,
        source::AcquireOptions,
        store::{ResourceLease, json_digest},
    },
    models::{
        policy::Freshness,
        retrieval::CachePolicy,
        store::{AcquisitionState, OutputEntry},
    },
    utils::lock::{LockGuard, LockMode},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

pub struct PreparedGroup {
    pub group: GroupRecord,
    /// Retained continuously from candidate preparation through publication.
    pub lease: GroupLease,
    pub pin: Option<crate::models::pin::PinRecord>,
    pub stale: BTreeSet<String>,
}
pub(super) struct Candidate {
    pub output: OutputEntry,
    pub guard: ResourceLease,
    pub check: Option<AcquisitionState>,
    pub stale: bool,
}
impl Store {
    pub fn prepare_group(
        &self,
        uuid: Uuid,
        expected_revision: Option<u64>,
        options: &AcquireOptions,
    ) -> Result<PreparedGroup> {
        self.prepare_group_inner(uuid, expected_revision, options, None)
    }
    pub fn prepare_group_with_pin(
        &self,
        uuid: Uuid,
        expected_revision: u64,
        options: &AcquireOptions,
        pin: &crate::models::pin::GroupPinRequest,
    ) -> Result<PreparedGroup> {
        self.prepare_group_inner(uuid, Some(expected_revision), options, Some(pin))
    }
    fn prepare_group_inner(
        &self,
        uuid: Uuid,
        expected_revision: Option<u64>,
        options: &AcquireOptions,
        pin: Option<&crate::models::pin::GroupPinRequest>,
    ) -> Result<PreparedGroup> {
        if let Some(pin) = pin {
            if let Some(receipt) = self.group_pin_retry(
                uuid,
                expected_revision.unwrap(),
                pin,
                crate::models::pin::GroupPinOperation::Prepare,
            )? {
                return self.open_pin_receipt(receipt);
            }
            let _lock = self.lock()?;
            self.validate_pin_policy(&crate::models::pin::PinRequest {
                registration_key: pin.registration_key.clone(),
                outputs: vec![],
                policy_refs: pin.policy_refs.clone(),
            })?;
        }
        let started = Instant::now();
        let mut plan = {
            let _lock = self.lock()?;
            let group = self.find_group(uuid)?;
            ensure!(
                expected_revision.is_none_or(|r| r == group.revision),
                Error::new(ErrorCode::RevisionConflict, "group revision changed")
            );
            let plan = self.plan_group(group, options, started)?;
            if pin.is_some() {
                self.authorize_group_pin_plan(&plan)?;
            }
            plan
        };
        let keys: BTreeSet<_> = plan.works.iter().map(|w| &w.intent).collect();
        let _intents = keys
            .into_iter()
            .map(|key| {
                LockGuard::acquire(
                    &self.root.join(format!(".locks/acquisition-{key}")),
                    LockMode::Exclusive,
                    Duration::from_secs(30).saturating_sub(started.elapsed()),
                )
            })
            .collect::<std::io::Result<Vec<_>>>()?;
        {
            let _lock = self.lock()?;
            self.check_group_plan(&plan)?;
        }
        let mut candidates: BTreeMap<usize, Candidate> = BTreeMap::new();
        let mut buckets: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, work) in plan.works.iter_mut().enumerate() {
            if !work.fetch {
                candidates.insert(
                    index,
                    Candidate {
                        output: work.old.clone().expect("selected output"),
                        guard: work.old_guard.take().expect("selected guard"),
                        check: None,
                        stale: false,
                    },
                );
            } else if work.exact {
                let execution = self.group_execution(work);
                let original = work.old.as_ref().expect("recovery output");
                let mut replay = original.clone();
                replay.request.adopted = work.request.adopted.clone();
                let prepared = execution.prepare_exact(&replay, options).map_err(|e| {
                    e.context(Error::new(
                        ErrorCode::ExactContentUnavailable,
                        "group exact source content is unavailable",
                    ))
                })?;
                let files = crate::core::store::manifest(&prepared.temp.path().join("content"))?;
                ensure!(
                    files == original.files
                        && prepared.resolved == original.resolved
                        && prepared.dependencies == original.dependencies,
                    Error::new(
                        ErrorCode::IntegrityError,
                        "group exact recovery differs from saved content"
                    )
                );
                let result =
                    execution.publish_prepared(&work.request, prepared, None, true, true)?;
                candidates.insert(
                    index,
                    Candidate {
                        output: result.acquired.entry,
                        guard: result.lease.expect("candidate guard"),
                        check: None,
                        stale: false,
                    },
                );
            } else {
                let mut identity = work.retrieval.clone();
                identity.provenance.clear();
                buckets
                    .entry(json_digest(&identity)?)
                    .or_default()
                    .push(index);
            }
        }
        // Share one exact retrieval handle for compatible requests. Drop it before
        // another request can contend on the same source's provider lock.
        for indexes in buckets.values() {
            let first = &plan.works[indexes[0]];
            let mut request = first.retrieval.clone();
            if indexes.iter().any(|i| plan.works[*i].due)
                && let CachePolicy::Cached { freshness, .. } = &mut request.cache
            {
                *freshness = Some(Freshness::Always {});
            }
            let execution = self.group_execution(first);
            let mut retrieved = execution
                .retrieve_authorized(
                    &request,
                    &RetrieveOptions {
                        test_now: options.test_now,
                        test_github_api: options.test_github_api.clone(),
                    },
                    first.authorization.clone(),
                )
                .map_err(|e| {
                    member_error(
                        e,
                        plan.views
                            .iter()
                            .filter(|(_, v)| indexes.contains(&v.work))
                            .map(|(name, _)| name.clone()),
                    )
                })?;
            for index in indexes {
                let work = &mut plan.works[*index];
                if retrieved.descriptor().stale && work.old_guard.is_some() {
                    candidates.insert(
                        *index,
                        Candidate {
                            output: work.old.clone().unwrap(),
                            guard: work.old_guard.take().unwrap(),
                            check: None,
                            stale: true,
                        },
                    );
                    continue;
                }
                let execution = self.group_execution(work);
                retrieved.authorization = work.authorization.clone();
                let result = execution
                    .prepare_acquisition_candidate(
                        &retrieved,
                        &work.acquisition,
                        work.key.clone(),
                        work.previous.as_ref(),
                    )
                    .map_err(|e| {
                        member_error(
                            e,
                            plan.views
                                .iter()
                                .filter(|(_, v)| v.work == *index)
                                .map(|(name, _)| name.clone()),
                        )
                    })?;
                candidates.insert(
                    *index,
                    Candidate {
                        output: result.acquired.entry,
                        guard: result.lease.expect("candidate guard"),
                        check: result.pending_check,
                        stale: result.acquired.stale,
                    },
                );
            }
        }
        // Verify all selected paths before entering the final metadata transaction.
        for (index, candidate) in &mut candidates {
            let mode = self.group_verification(&plan, &plan.works[*index])?;
            if mode == crate::models::store::VerificationMode::Full {
                candidate.guard.verify(self, &candidate.output)?;
            }
        }
        #[cfg(test)]
        if let Some(observer) = &self.before_claim_commit {
            observer();
        }
        self.publish_group(plan, candidates, pin)
    }
    fn group_execution(&self, work: &Work) -> Store {
        let mut store = self.clone();
        store.policy_fence = Some(work.fence.clone());
        store
    }
    pub(super) fn check_group_plan(&self, plan: &Plan) -> Result<()> {
        ensure!(
            self.find_group(plan.group.uuid)?.revision == plan.group.revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "group changed during preparation"
            )
        );
        self.check_group_owner(plan.group.owner)?;
        for work in &plan.works {
            work.fence.check(self)?;
            if let Some(auth) = &work.authorization {
                self.authorize_resource(
                    &self.registry(auth.owner.app)?,
                    work.action,
                    Some(work.request.source_uuid),
                    auth.owner.scope.as_ref(),
                )?;
            }
            ensure!(
                self.acquisition_by_key(&work.key)?
                    .as_ref()
                    .map(|s| (s.uuid, s.revision))
                    == work.previous.as_ref().map(|s| (s.uuid, s.revision)),
                Error::new(
                    ErrorCode::RevisionConflict,
                    "group acquisition check changed"
                )
            );
        }
        Ok(())
    }
    pub(super) fn group_verification(
        &self,
        plan: &Plan,
        work: &Work,
    ) -> Result<crate::models::store::VerificationMode> {
        use crate::core::policy::{
            persistence,
            resolve::{self, Inputs, Operation},
        };
        let _lock = self.lock()?;
        let source = self.source(work.request.source_uuid)?;
        let app = plan
            .group
            .owner
            .map(|id| self.registry(id).map(|r| r.registry.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let catalog = persistence::load(&self.metadata, &self.metadata.snapshot()?)?;
        let policies = resolve::evaluate(
            &catalog,
            Inputs {
                source: &source.policy_refs,
                application: &app,
                request: &Default::default(),
                pinned: None,
                operation: Operation::Read,
                managed_output: false,
                update_due: false,
            },
        )?;
        Ok(plan
            .boundary
            .verification(policies.behavior.path_verification))
    }
}
impl AppSession {
    pub fn prepare_group_with_pin(
        &self,
        uuid: Uuid,
        expected_revision: u64,
        options: &AcquireOptions,
        pin: &crate::models::pin::GroupPinRequest,
    ) -> Result<PreparedGroup> {
        self.store
            .prepare_group_with_pin(uuid, expected_revision, options, pin)
    }
    pub fn prepare_group(
        &self,
        uuid: Uuid,
        expected_revision: Option<u64>,
        options: &AcquireOptions,
    ) -> Result<PreparedGroup> {
        self.store.prepare_group(uuid, expected_revision, options)
    }
}
