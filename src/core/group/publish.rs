use super::{
    plan::Plan,
    prepare::{Candidate, PreparedGroup},
    *,
};
use crate::{
    core::lease::GroupLease,
    models::{
        group::{Generation, SelectedMember},
        lease::{GroupDescriptor, GroupMemberDescriptor},
    },
};
use std::collections::BTreeMap;

impl Store {
    pub(super) fn publish_group(
        &self,
        mut plan: Plan,
        mut candidates: BTreeMap<usize, Candidate>,
        pin_request: Option<&crate::models::pin::GroupPinRequest>,
    ) -> Result<PreparedGroup> {
        let _lock = self.lock()?;
        self.check_group_plan(&plan)?;
        let expected_revision = plan.group.revision;
        let mut journal = Journal::default();
        let mut registry = plan.group.owner.map(|id| self.registry(id)).transpose()?;
        let mut claims_changed = false;
        let mut selections = BTreeMap::new();
        let mut descriptor = GroupDescriptor {
            members: BTreeMap::new(),
        };
        let mut claim_outputs = BTreeMap::new();
        let mut stale = std::collections::BTreeSet::new();
        for (name, view) in &plan.views {
            let candidate = &candidates[&view.work];
            ensure!(
                view.required_files.iter().all(|path| candidate
                    .output
                    .files
                    .get(path)
                    .is_some_and(|entry| entry.digest.is_some())),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "covered group binary is absent from candidate manifest"
                )
            );
            self.metadata_path(&candidate.output)?;
            let work = &plan.works[view.work];
            if candidate.stale {
                stale.insert(name.clone());
            }
            let check = candidate.check.as_ref().or(work.previous.as_ref());
            if let Some(claim) = &work.claim {
                ensure!(
                    claim_outputs
                        .insert(claim.uuid, candidate.output.uuid)
                        .is_none_or(|old| old == candidate.output.uuid),
                    Error::new(
                        ErrorCode::ClaimOverlap,
                        "one group cannot select two outputs for the same claim"
                    )
                );
            }
            let mode = self.group_verification(&plan, work)?;
            let relative = view.relative.clone();
            let directory = candidate
                .guard
                .directory_view(&candidate.output, relative.as_deref())?;
            let mut provenance = plan.boundary.evidence.clone();
            if let Some(adopted) = &work.request.adopted {
                provenance.extend(adopted.evidence.values().cloned());
            }
            selections.insert(
                name.clone(),
                SelectedMember {
                    output_uuid: candidate.output.uuid,
                    claim_uuid: work.claim.as_ref().map(|c| c.uuid),
                    scope: work.claim.as_ref().map(|c| c.scope.clone()),
                    relative: relative.clone(),
                    policy_provenance: provenance.clone(),
                    selected_source: candidate.output.resolved.clone(),
                    checked_source: check.and_then(|s| s.checked_source.clone()),
                    last_success: check.and_then(|s| s.last_success),
                },
            );
            descriptor.members.insert(
                name.clone(),
                GroupMemberDescriptor {
                    output_uuid: candidate.output.uuid,
                    claim_uuid: work.claim.as_ref().map(|c| c.uuid),
                    directory,
                    relative,
                    verification: mode,
                    policy_provenance: provenance,
                },
            );
        }
        for (index, candidate) in &mut candidates {
            if let Some(check) = candidate.check.take() {
                journal.records.push(Record::Acquisition(Box::new(check)));
            }
            if let (Some(registry), Some(claim)) = (&mut registry, &plan.works[*index].claim)
                && plan.works[*index].publish_claim
            {
                let mut updated = claim.clone();
                updated.entry_uuid = candidate.output.uuid;
                let changed = registry
                    .claims
                    .get(&claim.uuid)
                    .is_none_or(|old| old.entry_uuid != updated.entry_uuid);
                if changed {
                    registry.claims.insert(claim.uuid, updated);
                    claims_changed = true;
                }
            }
        }
        let selection_changed = plan
            .group
            .selected
            .as_ref()
            .is_none_or(|s| s.members != selections);
        if selection_changed || !journal.records.is_empty() || claims_changed {
            plan.group.revision = plan
                .group
                .revision
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("group revision overflow"))?;
            plan.group.selected = Some(Generation {
                uuid: Uuid::new_v4(),
                members: selections,
            });
            journal.groups.push(plan.group.clone());
            if let Some(mut registry) = registry {
                let mut app = self.application(registry.registry.app)?;
                app.bump()?;
                registry.registry.revision = app.revision;
                journal.records.push(Record::Application(app));
                journal.registries.push(registry);
            }
        }
        let mut pin = pin_request
            .map(|request| {
                self.build_group_pin(
                    &plan.group,
                    expected_revision,
                    request,
                    crate::models::pin::GroupPinOperation::Prepare,
                )
            })
            .transpose()?;
        if let Some(pin) = &mut pin {
            pin.group.as_mut().unwrap().stale = stale.clone();
        }
        if let Some(pin) = &pin {
            journal.pins.push(pin.clone());
        }
        if !journal.groups.is_empty() || !journal.pins.is_empty() {
            self.commit_observed(journal, |_event| {
                #[cfg(test)]
                if let Some(observer) = &self.group_publication_observer {
                    observer(_event)?;
                }
                Ok(())
            })?;
        }
        let mut leases = BTreeMap::new();
        for (_, candidate) in candidates {
            leases
                .entry(candidate.output.uuid)
                .or_insert(candidate.guard);
        }
        Ok(PreparedGroup {
            group: plan.group,
            lease: GroupLease { leases, descriptor },
            pin,
            stale,
        })
    }
}
