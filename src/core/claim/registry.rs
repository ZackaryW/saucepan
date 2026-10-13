//! Registry snapshots are reconstructed from authenticated members. They are
//! transient coordinated observations, never a second persisted claim authority.
use super::{
    persistence::{Metadata, Snapshot, digest, record_path},
    policy::{self, Caller, Proof, Resource},
    security::{Authenticator, Identity, Kind, integrity_error},
};
use crate::{
    error::{Error, ErrorCode, Result},
    models::claim::{
        Claim, Registry, Scope,
        policy::Action,
        security::{Credential, CredentialPolicy},
    },
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub use crate::models::claim::RegistryState;

impl RegistryState {
    pub fn load(metadata: &Metadata, snapshot: &Snapshot, app: Uuid) -> Result<Self> {
        let mut registry: Registry =
            snapshot.read(metadata.authenticator(), registry_identity(app))?;
        let policies = crate::core::policy::persistence::load(metadata, snapshot)?;
        (registry.access, registry.policies) =
            crate::core::policy::adopt::application_settings(&policies, &registry.policy_refs)
                .map_err(|_| integrity_error())?;
        if metadata.authenticator().mode() == crate::models::claim::security::IntegrityMode::Basic
            && matches!(registry.credential, CredentialPolicy::Required { .. })
        {
            return Err(integrity_error());
        }
        registry.validate().map_err(|_| integrity_error())?;
        if registry.app != app {
            return Err(integrity_error());
        }
        let actual: BTreeSet<_> = snapshot
            .catalog
            .records
            .values()
            .filter(|reference| {
                reference.identity.kind == Kind::Claim && reference.identity.owner == Some(app)
            })
            .map(|reference| reference.identity.member.ok_or_else(integrity_error))
            .collect::<Result<_>>()?;
        if actual != registry.members.keys().copied().collect() {
            return Err(integrity_error());
        }
        let mut claims = BTreeMap::new();
        for (id, expected) in &registry.members {
            let identity = claim_identity(app, *id);
            if snapshot.digest(identity)? != expected {
                return Err(integrity_error());
            }
            let claim: Claim = snapshot.read(metadata.authenticator(), identity)?;
            if claim.uuid != *id {
                return Err(integrity_error());
            }
            claims.insert(*id, claim);
        }
        let state = Self { registry, claims };
        state.validate().map_err(|_| integrity_error())?;
        state
            .validate_policy_evidence(metadata, snapshot)
            .map_err(|_| integrity_error())?;
        Ok(state)
    }

    pub fn validate(&self) -> Result<()> {
        self.registry.validate()?;
        for (id, claim) in &self.claims {
            claim.validate()?;
            if claim.uuid != *id
                || (claim.request.adopted.is_none()
                    && self.registry.policies.evaluate(&claim.overrides)? != claim.effective)
                || claim.request.policies != claim.effective.output
            {
                return Err(Error::new(
                    ErrorCode::PolicyConflict,
                    "claim no longer satisfies registered policies",
                ));
            }
        }
        Ok(())
    }

    /// Encode all members and membership digests for one generation. The caller
    /// adds application/revision/binding changes to the same metadata publication.
    pub fn encode(&self, metadata: &Metadata, snapshot: &Snapshot) -> Result<RegistryWrites> {
        self.validate()?;
        self.validate_policy_evidence(metadata, snapshot)?;
        let generation = snapshot
            .catalog
            .generation
            .checked_add(1)
            .ok_or_else(integrity_error)?;
        let app = self.registry.app;
        let mut registry = self.registry.clone();
        registry.members.clear();
        let mut writes = Vec::new();
        for (id, claim) in &self.claims {
            let identity = claim_identity(app, *id);
            // Use the same Value representation as the publication writer, so
            // exact raw-byte signatures and membership digests agree.
            let value = serde_json::to_value(claim).map_err(|_| integrity_error())?;
            let bytes = metadata
                .authenticator()
                .sign(identity, generation, &value)?;
            registry.members.insert(*id, digest(&bytes));
            writes.push((identity, value));
        }
        writes.push((
            registry_identity(app),
            serde_json::to_value(registry).map_err(|_| integrity_error())?,
        ));
        let deletes = snapshot
            .catalog
            .records
            .values()
            .filter(|reference| {
                reference.identity.kind == Kind::Claim && reference.identity.owner == Some(app)
            })
            .filter(|reference| {
                reference
                    .identity
                    .member
                    .is_some_and(|id| !self.claims.contains_key(&id))
            })
            .map(|reference| reference.identity)
            .collect();
        Ok(RegistryWrites { writes, deletes })
    }

    fn validate_policy_evidence(&self, metadata: &Metadata, snapshot: &Snapshot) -> Result<()> {
        let catalog = crate::core::policy::persistence::load(metadata, snapshot)?;
        for id in self.registry.policy_refs.ids() {
            catalog.get(*id)?;
        }
        if !self.registry.policy_refs.ids().is_empty() {
            let empty = crate::models::policy::PolicyRefs::default();
            let plan = crate::core::policy::resolve::evaluate(
                &catalog,
                crate::core::policy::resolve::Inputs {
                    source: &empty,
                    application: &self.registry.policy_refs,
                    request: &empty,
                    pinned: None,
                    operation: crate::core::policy::resolve::Operation::Configure,
                    managed_output: true,
                    update_due: false,
                },
            )?;
            if crate::core::policy::adopt::access(&plan) != self.registry.access
                || crate::core::policy::adopt::registration(&plan)
                    .map_err(|error| Error::from_anyhow(&error))?
                    != self.registry.policies
            {
                return Err(integrity_error());
            }
        }
        for claim in self.claims.values() {
            for id in claim.policy_refs.ids() {
                catalog.get(*id)?;
            }
            if let Some(adopted) = &claim.request.adopted {
                let source: crate::models::source::SourceRecord = snapshot.read(
                    metadata.authenticator(),
                    Identity {
                        kind: Kind::Source,
                        owner: None,
                        member: Some(claim.request.source_uuid),
                    },
                )?;
                let plan = crate::core::policy::claims::surviving(
                    &catalog,
                    adopted,
                    &source.policy_refs,
                    &self.registry.policy_refs,
                )
                .map_err(|error| Error::from_anyhow(&error))?;
                if crate::core::policy::adopt::effective(&plan)
                    .map_err(|error| Error::from_anyhow(&error))?
                    != claim.effective
                    || crate::core::policy::claims::behavior_refs(&catalog, adopted)
                        .map_err(|error| Error::from_anyhow(&error))?
                        != claim.policy_refs
                {
                    return Err(integrity_error());
                }
                if crate::core::policy::resolve::filters(&catalog, &adopted.filter_refs)?
                    != claim.request.selection.filters
                    || adopted.behavior.selection != claim.request.selection.selection_policy
                    || adopted.behavior.cache != crate::models::policy::record::CacheMode::Cached
                {
                    return Err(integrity_error());
                }
            }
        }
        Ok(())
    }

    pub fn authorize(
        &self,
        auth: &Authenticator,
        token: Option<&Credential>,
        action: Action,
        source: Option<Uuid>,
        scope: Option<&Scope>,
    ) -> Result<()> {
        auth.check_credential(self.registry.app, &self.registry.credential, token)?;
        policy::authorize(
            &self.registry.access,
            self.credentials_required(),
            Caller::Application {
                app: self.registry.app,
                proof: Proof::Verified,
            },
            action,
            &Resource {
                app: self.registry.app,
                source,
                scope,
            },
        )
    }

    pub fn publicly_visible(&self, claim: &Claim) -> bool {
        policy::authorize(
            &self.registry.access,
            self.credentials_required(),
            Caller::PublicDiscovery,
            Action::Discover,
            &Resource {
                app: self.registry.app,
                source: Some(claim.request.source_uuid),
                scope: Some(&claim.scope),
            },
        )
        .is_ok()
            && self
                .registry
                .access
                .actions
                .as_ref()
                .is_none_or(|actions| actions.contains(&Action::Read))
    }

    pub fn credentials_required(&self) -> bool {
        !matches!(self.registry.credential, CredentialPolicy::Disabled)
    }

    /// Called only by trusted registration administration. Verifies current proof
    /// independently of app operation allowances; it is not an app request flag.
    pub fn change_credentials(
        &mut self,
        auth: &Authenticator,
        current: Option<&Credential>,
        change: CredentialChange,
    ) -> Result<Option<Credential>> {
        auth.check_credential(self.registry.app, &self.registry.credential, current)?;
        let policy = match (&self.registry.credential, change) {
            (CredentialPolicy::Disabled, CredentialChange::Disable) => return Ok(None),
            (CredentialPolicy::Disabled, CredentialChange::Enable) => 1,
            (
                CredentialPolicy::Required { generation, .. }
                | CredentialPolicy::Bearer { generation, .. },
                CredentialChange::Rotate,
            ) => generation.checked_add(1).ok_or_else(integrity_error)?,
            (
                CredentialPolicy::Required { .. } | CredentialPolicy::Bearer { .. },
                CredentialChange::Disable,
            ) => {
                self.registry.credential = CredentialPolicy::Disabled;
                return Ok(None);
            }
            _ => {
                return Err(Error::new(
                    ErrorCode::InvalidRequest,
                    "credential transition does not match current mode",
                ));
            }
        };
        let token = Credential::generate()?;
        self.registry.credential = auth.credential_policy(self.registry.app, policy, &token)?;
        Ok(Some(token))
    }

    pub fn retire(snapshot: &Snapshot, app: Uuid) -> Result<Vec<Identity>> {
        let registry = registry_identity(app);
        if !snapshot.contains(registry)? {
            return Err(integrity_error());
        }
        let mut identities = vec![registry];
        identities.extend(
            snapshot
                .catalog
                .records
                .values()
                .filter(|reference| {
                    reference.identity.kind == Kind::Claim && reference.identity.owner == Some(app)
                })
                .map(|reference| reference.identity),
        );
        for identity in &identities {
            record_path(*identity)?;
        }
        Ok(identities)
    }
}

pub struct RegistryWrites {
    pub writes: Vec<(Identity, Value)>,
    pub deletes: Vec<Identity>,
}
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialChange {
    Enable,
    Rotate,
    Disable,
}
pub fn registry_identity(app: Uuid) -> Identity {
    Identity {
        kind: Kind::Registry,
        owner: Some(app),
        member: None,
    }
}
pub fn claim_identity(app: Uuid, claim: Uuid) -> Identity {
    Identity {
        kind: Kind::Claim,
        owner: Some(app),
        member: Some(claim),
    }
}
