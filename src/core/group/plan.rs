//! Resolve the complete set before provider effects; choose containing fresh claims first.
use super::{intent::MemberIntent, *};
use crate::{
    core::{
        claim::{composition::Authorization, matching},
        policy::{fence::PolicyFence, group::GroupBoundary, persistence},
        source::AcquireOptions,
        store::{ResourceLease, json_digest, updates::scoped_intent_key},
    },
    models::{
        claim::{Claim, policy::ReclaimStrategy},
        group::Target,
        policy::Identical,
        retrieval::RetrievalPlan,
        store::{AcquisitionPlan, AcquisitionState, OutputEntry, OutputPlan},
    },
};

use std::collections::BTreeMap;

pub(super) struct Work {
    pub request: OutputPlan,
    pub retrieval: RetrievalPlan,
    pub acquisition: AcquisitionPlan,
    pub authorization: Option<Authorization>,
    pub claim: Option<Claim>,
    pub previous: Option<AcquisitionState>,
    pub key: String,
    pub intent: String,
    pub old: Option<OutputEntry>,
    pub old_guard: Option<ResourceLease>,
    pub fetch: bool,
    pub due: bool,
    pub exact: bool,
    pub action: Action,
    pub publish_claim: bool,
    pub fence: PolicyFence,
}
pub(super) struct View {
    pub work: usize,
    pub relative: Option<String>,
}
pub(super) struct Plan {
    pub group: GroupRecord,
    pub works: Vec<Work>,
    pub views: BTreeMap<String, View>,
    pub boundary: GroupBoundary,
}
impl Store {
    pub(super) fn plan_group(
        &self,
        group: GroupRecord,
        options: &AcquireOptions,
        started: std::time::Instant,
    ) -> Result<Plan> {
        self.check_group_owner(group.owner)?;
        let catalog = persistence::load(&self.metadata, &self.metadata.snapshot()?)?;
        let boundary = GroupBoundary::evaluate(&catalog, &group.definition.policy_refs)?;
        let mut members: Vec<_> = group.definition.members.iter().collect();
        // A containing acquisition owns fresh covered intents regardless of input order.
        members.sort_by_key(|m| match &m.target {
            Target::Acquire(a) => (
                0,
                a.selection
                    .folder
                    .as_ref()
                    .map_or(0, |s| s.split('/').count()),
                m.name.clone(),
            ),
            Target::Claim { .. } => (1, 0, m.name.clone()),
            Target::Output { .. } => (2, 0, m.name.clone()),
        });
        let mut works: Vec<Work> = Vec::new();
        let mut views = BTreeMap::new();
        for member in members {
            let MemberIntent {
                mut request,
                mut claim,
                mut relative,
                fixed,
            } = self.group_member_request(&group, member)?;
            let mut shared = None;
            for (index, work) in works.iter().enumerate() {
                let same_claim = claim
                    .as_ref()
                    .zip(work.claim.as_ref())
                    .is_some_and(|(a, b)| a.uuid == b.uuid);
                let scope = claim.as_ref().map(|c| &c.scope);
                let mut left = work.request.selection.clone();
                left.folder = None;
                let mut right = request.selection.clone();
                right.folder = None;
                let same_family = scope == work.claim.as_ref().map(|c| &c.scope)
                    && request.source_uuid == work.request.source_uuid
                    && request.selector == work.request.selector
                    && left == right;
                let covered = matching::projection(
                    work.request.selection.folder.as_deref(),
                    request.selection.folder.as_deref(),
                );
                if !matches!(member.target, Target::Output { .. })
                    && (same_claim
                        || (same_family
                            && (request.selection.folder == work.request.selection.folder
                                || covered.is_some())
                            && matches!(member.target, Target::Acquire(_))))
                {
                    ensure!(
                        request.policies == work.request.policies
                            && request.adopted.as_ref().map(|a| &a.behavior)
                                == work.request.adopted.as_ref().map(|a| &a.behavior)
                            && claim.as_ref().map(|c| &c.context)
                                == work.claim.as_ref().map(|c| &c.context),
                        Error::new(
                            ErrorCode::ClaimOverlap,
                            "overlapping group intents have different context or policies"
                        )
                    );
                    if !same_claim {
                        relative = join_relative(covered.as_deref(), relative.as_deref());
                    }
                    shared = Some(index);
                    break;
                }
            }
            let index = if let Some(index) = shared {
                index
            } else {
                let owner = group.owner;
                let (retrieval, acquisition) = self.composition_requests(&request, options)?;
                let mut authorization = owner
                    .map(|app| {
                        Ok::<_, anyhow::Error>(Authorization {
                            policy_fence: PolicyFence::capture(
                                self,
                                Some(request.source_uuid),
                                Some(app),
                            )?,
                            owner: crate::models::store::acquisition::AcquisitionOwner {
                                app,
                                scope: claim.as_ref().map(|c| c.scope.clone()),
                                claim: claim.as_ref().map(|c| c.uuid),
                            },
                            revision: self.application(app)?.revision,
                            update: false,
                            action: Action::Acquire,
                        })
                    })
                    .transpose()?;
                if let Some(auth) = &mut authorization {
                    auth.owner.claim = claim.as_ref().map(|c| c.uuid);
                }
                let key = scoped_intent_key(&retrieval, &acquisition, authorization.as_ref())?;
                let previous = self.acquisition_by_key(&key)?;
                let old_id = fixed
                    .or_else(|| {
                        claim
                            .as_ref()
                            .filter(|c| c.entry_uuid != Uuid::nil())
                            .map(|c| c.entry_uuid)
                    })
                    .or_else(|| previous.as_ref().map(|p| p.selected));
                let old = old_id.map(|id| self.output(id)).transpose()?;
                let available = old.as_ref().map(|e| self.metadata_path(e)).transpose();
                let available = match available {
                    Ok(path) => path.is_some(),
                    Err(e)
                        if Error::from_anyhow(&e).code == ErrorCode::ContentUnavailable
                            && fixed.is_none() =>
                    {
                        false
                    }
                    Err(e) => return Err(e),
                };
                let missing = old.is_some() && !available;
                let mut exact = false;
                let due = matches!(member.target, Target::Acquire(_))
                    && acquisition.update_policy.due(
                        previous.as_ref().and_then(|p| p.last_success),
                        options.now()?,
                    );
                let fetch = fixed.is_none()
                    && (old.is_none()
                        || missing
                        || (matches!(member.target, Target::Acquire(_))
                            && (due || acquisition.identical == Identical::NewEntry)));
                let action = if missing && claim.is_some() {
                    Action::Recover
                } else if fetch && old.is_some() {
                    Action::Update
                } else if fetch {
                    Action::Acquire
                } else {
                    Action::Lease
                };
                if missing && let Some(claim) = &claim {
                    ensure!(
                        claim.effective.reclaim.strategy() != ReclaimStrategy::ReportUnavailable,
                        Error::new(
                            ErrorCode::ContentUnavailable,
                            "group claim content is unavailable"
                        )
                    );
                    exact = claim.effective.reclaim.strategy() == ReclaimStrategy::RestoreExact;
                    request = claim.request.clone();
                }
                if let Some(auth) = &mut authorization {
                    auth.update = action == Action::Update;
                    if action == Action::Recover {
                        auth.action = Action::Recover;
                    }
                    self.authorize_resource(
                        &self.registry(auth.owner.app)?,
                        action,
                        Some(request.source_uuid),
                        auth.owner.scope.as_ref(),
                    )?;
                } else {
                    for action in [action, Action::Lease] {
                        self.authorize_policy_refs(
                            &Default::default(),
                            Some(request.source_uuid),
                            None,
                            action,
                            crate::core::policy::authority::Projection::Resource,
                        )?;
                    }
                }
                boundary.authorize_action(
                    action,
                    request.source_uuid,
                    claim.as_ref().map(|c| &c.scope),
                )?;
                boundary.authorize_action(
                    Action::Lease,
                    request.source_uuid,
                    claim.as_ref().map(|c| &c.scope),
                )?;
                if let Some(app) = owner {
                    self.authorize_resource(
                        &self.registry(app)?,
                        Action::Lease,
                        Some(request.source_uuid),
                        claim.as_ref().map(|c| &c.scope),
                    )?;
                }
                crate::core::policy::composition::validate_composition(
                    &retrieval,
                    &acquisition,
                    due,
                )?;
                let intent = if let (Some(app), Some(claim)) = (owner, &claim) {
                    json_digest(&(
                        "claim-update",
                        app,
                        &claim.scope,
                        request.source_uuid,
                        &request.selector,
                    ))?
                } else {
                    key.clone()
                };
                let refs: Vec<_> = group
                    .definition
                    .policy_refs
                    .ids()
                    .iter()
                    .chain(
                        request
                            .adopted
                            .as_ref()
                            .into_iter()
                            .flat_map(|a| a.evidence.values().map(|e| &e.uuid)),
                    )
                    .copied()
                    .collect();
                let fence =
                    PolicyFence::capture_references(self, Some(request.source_uuid), owner, &refs)?;
                let index = works.len();
                works.push(Work {
                    request,
                    retrieval,
                    acquisition,
                    authorization,
                    claim: claim.take(),
                    previous,
                    key,
                    intent,
                    old,
                    old_guard: None,
                    fetch,
                    due,
                    exact,
                    action,
                    publish_claim: !matches!(member.target, Target::Output { .. }),
                    fence,
                });
                index
            };
            views.insert(
                member.name.clone(),
                View {
                    work: index,
                    relative: join_relative(relative.as_deref(), member.relative.as_deref()),
                },
            );
        }
        let mut retained = Vec::new();
        for (index, work) in works.iter().enumerate() {
            if let Some(old) = &work.old {
                match self.metadata_path(old) {
                    Ok(_) => retained.push((old.uuid, index)),
                    Err(e)
                        if work.fetch
                            && Error::from_anyhow(&e).code == ErrorCode::ContentUnavailable => {}
                    Err(e) => return Err(e),
                }
            }
        }
        retained.sort();
        for (_, index) in retained {
            let work = &mut works[index];
            work.old_guard = Some(self.metadata_lease_with_timeout(
                work.old.as_ref().unwrap(),
                std::time::Duration::from_secs(30).saturating_sub(started.elapsed()),
            )?);
        }
        Ok(Plan {
            group,
            works,
            views,
            boundary,
        })
    }
}
pub(super) fn join_relative(a: Option<&str>, b: Option<&str>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(format!("{a}/{b}")),
        (Some(a), None) | (None, Some(a)) => Some(a.into()),
        _ => None,
    }
}
