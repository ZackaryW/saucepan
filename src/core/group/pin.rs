use super::*;
use crate::{
    core::{
        lease::GroupLease,
        policy::{
            fence::PolicyFence,
            group::GroupBoundary,
            persistence,
            resolve::{self, Inputs, Operation},
        },
    },
    models::{
        lease::{GroupDescriptor, GroupMemberDescriptor},
        pin::{
            GroupPinOperation, GroupPinReceipt, GroupPinRequest, PinRecord, PinRequest, PinState,
            PinWitness,
        },
        store::VerificationMode,
    },
};
use anyhow::Context;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

impl Store {
    pub(super) fn authorize_group_pin_plan(&self, plan: &plan::Plan) -> Result<()> {
        ensure!(
            plan.group.owner == self.session.as_ref().map(|s| s.app),
            Error::new(
                ErrorCode::AccessDenied,
                "pin preparation requires the group's explicit owner"
            )
        );
        for work in &plan.works {
            let scope = work.claim.as_ref().map(|c| &c.scope);
            plan.boundary
                .authorize_action(Action::Pin, work.request.source_uuid, scope)?;
            if let Some(app) = plan.group.owner {
                for action in [Action::Pin, Action::Read] {
                    self.authorize_resource(
                        &self.registry(app)?,
                        action,
                        Some(work.request.source_uuid),
                        scope,
                    )?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn group_pin_retry(
        &self,
        uuid: Uuid,
        revision: u64,
        request: &GroupPinRequest,
        operation: GroupPinOperation,
    ) -> Result<Option<PinRecord>> {
        let _lock = self.lock()?;
        let owner = self.session.as_ref().map(|s| s.app);
        let receipt = self
            .pin_records_locked()?
            .into_iter()
            .find(|p| p.owner == owner && p.request.registration_key == request.registration_key);
        if let Some(pin) = &receipt {
            ensure!(
                pin.state == PinState::Active
                    && pin.request.policy_refs == request.policy_refs
                    && pin.group.as_ref().is_some_and(|r| r.group.uuid == uuid
                        && r.expected_revision == revision
                        && r.operation == operation),
                Error::new(
                    ErrorCode::RevisionConflict,
                    "pin key belongs to a different or released operation"
                )
            );
            self.authorize_pin(pin, Action::Pin)?;
            self.authorize_pin(pin, Action::Read)?;
        }
        Ok(receipt)
    }
    /// Caller retains all selected outputs and holds metadata coordination.
    pub(super) fn build_group_pin(
        &self,
        group: &GroupRecord,
        expected_revision: u64,
        request: &GroupPinRequest,
        operation: GroupPinOperation,
    ) -> Result<PinRecord> {
        ensure!(
            group.owner == self.session.as_ref().map(|s| s.app),
            Error::new(ErrorCode::AccessDenied, "pin requires group owner")
        );
        ensure!(
            self.group_pin_retry(group.uuid, expected_revision, request, operation)?
                .is_none(),
            Error::new(
                ErrorCode::RevisionConflict,
                "pin receipt already committed; retry by key"
            )
        );
        let selected = group.selected.as_ref().context("group has no selection")?;
        let catalog = persistence::load(&self.metadata, &self.metadata.snapshot()?)?;
        let boundary = GroupBoundary::evaluate(&catalog, &group.definition.policy_refs)?;
        let mut witnesses = BTreeMap::new();
        for member in selected.members.values() {
            let output = self.output(member.output_uuid)?;
            self.metadata_path(&output)?;
            boundary.authorize_action(
                Action::Pin,
                output.request.source_uuid,
                member.scope.as_ref(),
            )?;
            let witness = PinWitness {
                output_uuid: output.uuid,
                source_uuid: output.request.source_uuid,
                scope: member.scope.clone(),
            };
            if let Some(app) = group.owner {
                ensure!(
                    member.claim_uuid.is_some() && member.scope.is_some(),
                    Error::new(ErrorCode::AccessDenied, "app pin lacks claim witness")
                );
                for action in [Action::Pin, Action::Read] {
                    self.authorize_resource(
                        &self.registry(app)?,
                        action,
                        Some(witness.source_uuid),
                        witness.scope.as_ref(),
                    )?;
                }
            }
            witnesses.entry(output.uuid).or_insert(witness);
        }
        let request = PinRequest {
            registration_key: request.registration_key.clone(),
            outputs: witnesses.keys().copied().collect(),
            policy_refs: request.policy_refs.clone(),
        };
        request.validate()?;
        self.validate_pin_policy(&request)?;
        Ok(PinRecord {
            uuid: Uuid::new_v4(),
            owner: group.owner,
            request,
            witnesses: witnesses.into_values().collect(),
            state: PinState::Active,
            group: Some(GroupPinReceipt {
                operation,
                expected_revision,
                group: group.clone(),
                stale: Default::default(),
            }),
        })
    }
    pub fn pin_group(
        &self,
        uuid: Uuid,
        expected_revision: u64,
        request: &GroupPinRequest,
    ) -> Result<PinRecord> {
        if let Some(pin) =
            self.group_pin_retry(uuid, expected_revision, request, GroupPinOperation::Pin)?
        {
            return Ok(pin);
        }
        let retained = self.open_group(uuid)?;
        let _lock = self.lock()?;
        ensure!(
            retained.group.revision == expected_revision
                && self.find_group(uuid)?.revision == expected_revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "group revision changed before pin"
            )
        );
        let pin = self.build_group_pin(
            &retained.group,
            expected_revision,
            request,
            GroupPinOperation::Pin,
        )?;
        self.commit(Journal {
            pins: vec![pin.clone()],
            ..Default::default()
        })?;
        Ok(pin)
    }
    /// Lost-response retries use the durable receipt, even when live claims moved.
    pub(super) fn open_pin_receipt(&self, pin: PinRecord) -> Result<PreparedGroup> {
        let started = Instant::now();
        let remaining = || Duration::from_secs(30).saturating_sub(started.elapsed());
        let group = pin
            .group
            .as_ref()
            .context("pin has no group receipt")?
            .group
            .clone();
        let (mut leases, outputs, modes, fences, descriptor) = {
            let _lock = self.lock_with_timeout(remaining())?;
            self.authorize_pin(&pin, Action::Read)?;
            let catalog = persistence::load(&self.metadata, &self.metadata.snapshot()?)?;
            let boundary = GroupBoundary::evaluate(&catalog, &group.definition.policy_refs)?;
            let app_refs = pin
                .owner
                .map(|id| self.registry(id).map(|r| r.registry.policy_refs))
                .transpose()?
                .unwrap_or_default();
            let mut outputs = BTreeMap::new();
            let mut modes = BTreeMap::new();
            let mut fences = Vec::new();
            let mut leases = BTreeMap::new();
            for witness in &pin.witnesses {
                let output = self.output(witness.output_uuid)?;
                let source = self.source(witness.source_uuid)?;
                let plan = resolve::evaluate(
                    &catalog,
                    Inputs {
                        source: &source.policy_refs,
                        application: &app_refs,
                        request: &Default::default(),
                        pinned: None,
                        operation: Operation::Read,
                        managed_output: false,
                        update_due: false,
                    },
                )?;
                modes.insert(
                    output.uuid,
                    boundary.verification(plan.behavior.path_verification),
                );
                fences.push(PolicyFence::capture(self, Some(source.uuid), pin.owner)?);
                leases.insert(
                    output.uuid,
                    self.metadata_lease_with_timeout(&output, remaining())?,
                );
                outputs.insert(output.uuid, output);
            }
            let mut descriptor = GroupDescriptor {
                members: BTreeMap::new(),
            };
            for (name, member) in &group
                .selected
                .as_ref()
                .context("receipt has no generation")?
                .members
            {
                let output = outputs
                    .get(&member.output_uuid)
                    .context("receipt output mismatch")?;
                boundary.authorize_action(
                    Action::Pin,
                    output.request.source_uuid,
                    member.scope.as_ref(),
                )?;
                if let Some(app) = pin.owner {
                    for action in [Action::Pin, Action::Read] {
                        self.authorize_resource(
                            &self.registry(app)?,
                            action,
                            Some(output.request.source_uuid),
                            member.scope.as_ref(),
                        )?;
                    }
                }
                descriptor.members.insert(
                    name.clone(),
                    GroupMemberDescriptor {
                        output_uuid: output.uuid,
                        claim_uuid: member.claim_uuid,
                        directory: leases[&output.uuid]
                            .directory_view(output, member.relative.as_deref())?,
                        relative: member.relative.clone(),
                        verification: modes[&output.uuid],
                        policy_provenance: member.policy_provenance.clone(),
                    },
                );
            }
            (leases, outputs, modes, fences, descriptor)
        };
        for (id, lease) in &mut leases {
            if modes[id] == VerificationMode::Full {
                lease.verify(self, &outputs[id])?;
            }
        }
        let _lock = self.lock_with_timeout(remaining())?;
        for fence in fences {
            fence.check(self)?;
        }
        ensure!(
            self.pin(pin.uuid)?.state == PinState::Active,
            Error::new(ErrorCode::RevisionConflict, "pin was released during retry")
        );
        Ok(PreparedGroup {
            group,
            stale: pin.group.as_ref().unwrap().stale.clone(),
            pin: Some(pin),
            lease: GroupLease { leases, descriptor },
        })
    }
}
impl AppSession {
    pub fn pin_group(
        &self,
        uuid: Uuid,
        expected_revision: u64,
        request: &GroupPinRequest,
    ) -> Result<PinRecord> {
        self.store.pin_group(uuid, expected_revision, request)
    }
}
