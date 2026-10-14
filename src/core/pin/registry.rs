use super::*;
use crate::{
    core::policy::persistence,
    models::{pin::PinWitness, policy::PolicyDefinition},
};
use anyhow::Context;
impl Store {
    pub(crate) fn pin_records_locked(&self) -> Result<Vec<PinRecord>> {
        let snapshot = self.metadata.snapshot()?;
        snapshot
            .catalog
            .records
            .values()
            .filter(|r| r.identity.kind == Kind::Pin)
            .map(|r| {
                let pin: PinRecord = snapshot.read(self.metadata.authenticator(), r.identity)?;
                pin.request.validate()?;
                ensure!(
                    identity(&pin) == r.identity
                        && pin.request == pin.request.canonical()
                        && pin.witnesses.len() == pin.request.outputs.len()
                        && pin
                            .witnesses
                            .iter()
                            .zip(&pin.request.outputs)
                            .all(|(w, id)| w.output_uuid == *id
                                && w.scope.is_some() == pin.owner.is_some()),
                    Error::new(
                        ErrorCode::IntegrityError,
                        "invalid pin identity or witnesses"
                    )
                );
                self.validate_pin_policy(&pin.request)?;
                Ok(pin)
            })
            .collect()
    }
    pub(super) fn find_pin(&self, uuid: Uuid) -> Result<PinRecord> {
        self.pin_records_locked()?
            .into_iter()
            .find(|p| p.uuid == uuid)
            .context(Error::new(ErrorCode::InvalidRequest, "unknown pin"))
    }
    pub(super) fn validate_pin_policy(&self, request: &PinRequest) -> Result<()> {
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        ensure!(
            request.policy_refs.ids().len() == 1,
            Error::new(
                ErrorCode::InvalidRequest,
                "pin requires exactly one registered pin_policy"
            )
        );
        ensure!(
            matches!(
                catalog.get(request.policy_refs.ids()[0])?.definition,
                PolicyDefinition::PinPolicy { .. }
            ),
            Error::new(ErrorCode::InvalidRequest, "pin accepts only pin_policy")
        );
        Ok(())
    }
    pub(super) fn pin_witness(&self, output: Uuid) -> Result<PinWitness> {
        let entry = self.output(output)?;
        self.metadata_path(&entry)?;
        let scope = if let Some(session) = &self.session {
            self.application(session.app)?;
            let registry = self.registry(session.app)?;
            self.check_session(&registry)?;
            let claim = registry
                .claims
                .values()
                .find(|c| {
                    c.entry_uuid == output
                        && self.authorize_claim(&registry, c, Action::Pin).is_ok()
                        && self.authorize_claim(&registry, c, Action::Read).is_ok()
                })
                .context(Error::new(
                    ErrorCode::AccessDenied,
                    "pin requires a readable, pinnable owned claim",
                ))?;
            Some(claim.scope.clone())
        } else {
            None
        };
        Ok(PinWitness {
            output_uuid: output,
            source_uuid: entry.request.source_uuid,
            scope,
        })
    }
    pub(super) fn authorize_pin(&self, pin: &PinRecord, action: Action) -> Result<()> {
        if let Some(session) = &self.session {
            ensure!(
                pin.owner == Some(session.app),
                Error::new(ErrorCode::AccessDenied, "pin owner mismatch")
            );
            self.application(session.app)?;
            let registry = self.registry(session.app)?;
            self.check_session(&registry)?;
            for witness in &pin.witnesses {
                self.authorize_resource(
                    &registry,
                    action,
                    Some(witness.source_uuid),
                    witness.scope.as_ref(),
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn pin_paths(&self, pin: PinRecord) -> Result<PinnedOutputs> {
        let paths = pin
            .request
            .outputs
            .iter()
            .map(|id| Ok((*id, self.metadata_path(&self.output(*id)?)?)))
            .collect::<Result<_>>()?;
        Ok(PinnedOutputs { pin, paths })
    }
}
