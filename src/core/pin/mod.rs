//! Explicit durable retention, published only after every target is admitted.
mod registry;
#[cfg(test)]
mod tests;
use crate::{
    core::{
        Store,
        claim::{
            security::{Identity, Kind},
            session::AppSession,
        },
        index::Journal,
        policy::{catalog::PolicyCatalog, fence::PolicyFence},
    },
    error::{Error, ErrorCode},
    models::{
        claim::policy::Action,
        lease::{GroupMember, GroupRequest, GroupTarget},
        pin::{PinRecord, PinRequest, PinState, PinnedOutputs},
    },
};
use anyhow::{Result, ensure};
use uuid::Uuid;

pub(crate) fn identity(pin: &PinRecord) -> Identity {
    Identity {
        kind: Kind::Pin,
        owner: pin.owner,
        member: Some(pin.uuid),
    }
}

impl Store {
    /// Retain exactly these immutable outputs until explicit release. No claim is created.
    pub fn pin_outputs(&self, request: &PinRequest) -> Result<PinnedOutputs> {
        request.validate()?;
        let started = std::time::Instant::now();
        let remaining = || std::time::Duration::from_secs(30).saturating_sub(started.elapsed());
        let request = request.canonical();
        let (pin, fences) = {
            let _lock = self.lock_with_timeout(remaining())?;
            self.validate_pin_policy(&request)?;
            let owner = self.session.as_ref().map(|s| s.app);
            if let Some(previous) = self.pin_records_locked()?.into_iter().find(|p| {
                p.owner == owner && p.request.registration_key == request.registration_key
            }) {
                self.authorize_pin(&previous, Action::Pin)?;
                self.authorize_pin(&previous, Action::Read)?;
                ensure!(
                    previous.request == request && previous.state == PinState::Active,
                    Error::new(
                        ErrorCode::RevisionConflict,
                        "pin key already has a different or released receipt"
                    )
                );
                return self.pin_paths(previous);
            }
            let witnesses = request
                .outputs
                .iter()
                .map(|id| self.pin_witness(*id))
                .collect::<Result<Vec<_>>>()?;
            let fences = witnesses
                .iter()
                .map(|w| {
                    PolicyFence::capture_references(
                        self,
                        Some(w.source_uuid),
                        owner,
                        request.policy_refs.ids(),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            (
                PinRecord {
                    uuid: Uuid::new_v4(),
                    owner,
                    request,
                    witnesses,
                    state: PinState::Active,
                    group: None,
                },
                fences,
            )
        };
        // Temporary shared guards bridge verification to the durable metadata commit.
        let group = GroupRequest {
            members: pin
                .request
                .outputs
                .iter()
                .enumerate()
                .map(|(n, id)| GroupMember {
                    name: format!("output_{n}"),
                    target: GroupTarget::Output { uuid: *id },
                    relative: None,
                })
                .collect(),
            policy_refs: Default::default(),
        };
        let guards =
            self.lease_group_action(&group, &PolicyCatalog::default(), Action::Pin, started)?;
        let _lock = self.lock_with_timeout(remaining())?;
        for fence in fences {
            fence.check(self)?;
        }
        self.authorize_pin(&pin, Action::Pin)?;
        // A concurrent equivalent registration converges; divergent keys never overwrite.
        if let Some(previous) = self.pin_records_locked()?.into_iter().find(|p| {
            p.owner == pin.owner && p.request.registration_key == pin.request.registration_key
        }) {
            ensure!(
                previous.request == pin.request && previous.state == PinState::Active,
                Error::new(ErrorCode::RevisionConflict, "pin registration key conflict")
            );
            return self.pin_paths(previous);
        }
        self.commit(Journal {
            pins: vec![pin.clone()],
            ..Default::default()
        })?;
        let result = self.pin_paths(pin)?;
        drop(guards);
        Ok(result)
    }

    /// Inspect using captured source/scope witnesses, even after the claim moves.
    pub fn pin(&self, uuid: Uuid) -> Result<PinRecord> {
        let _lock = self.lock()?;
        let pin = self.find_pin(uuid)?;
        self.authorize_pin(&pin, Action::Read)?;
        Ok(pin)
    }
    /// App sessions see only their own authorized pins; trusted administration sees all owners.
    pub fn pins(&self, key: Option<&str>) -> Result<Vec<PinRecord>> {
        let _lock = self.lock()?;
        if let Some(session) = &self.session {
            self.check_session(&self.registry(session.app)?)?;
        }
        Ok(self
            .pin_records_locked()?
            .into_iter()
            .filter(|p| {
                key.is_none_or(|key| p.request.registration_key.as_str() == key)
                    && self.authorize_pin(p, Action::Read).is_ok()
            })
            .collect())
    }
    /// Release is metadata-only: missing payloads and moved claims do not prevent cleanup.
    pub fn unpin(&self, uuid: Uuid) -> Result<PinRecord> {
        let _lock = self.lock()?;
        let mut pin = self.find_pin(uuid)?;
        self.authorize_pin(&pin, Action::Unpin)?;
        if pin.state == PinState::Active {
            pin.state = PinState::Released;
            self.commit(Journal {
                pins: vec![pin.clone()],
                ..Default::default()
            })?;
        }
        Ok(pin)
    }
}
impl AppSession {
    pub fn pin_outputs(&self, request: &PinRequest) -> Result<PinnedOutputs> {
        self.store.pin_outputs(request)
    }
    pub fn pin(&self, uuid: Uuid) -> Result<PinRecord> {
        self.store.pin(uuid)
    }
    pub fn pins(&self, key: Option<&str>) -> Result<Vec<PinRecord>> {
        self.store.pins(key)
    }
    pub fn unpin(&self, uuid: Uuid) -> Result<PinRecord> {
        self.store.unpin(uuid)
    }
}
