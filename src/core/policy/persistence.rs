//! Policy definitions use the same catalog and journal as their consumers.
use super::catalog::PolicyCatalog;
use crate::{
    core::{
        Store,
        claim::{
            persistence::{Metadata, Snapshot},
            security::{Identity, Kind, integrity_error},
        },
    },
    error::{Error, ErrorCode, Result},
    models::policy::PolicyRecord,
};
use anyhow::ensure;
use uuid::Uuid;

pub(crate) fn identity(uuid: Uuid) -> Identity {
    Identity {
        kind: Kind::Policy,
        owner: None,
        member: Some(uuid),
    }
}

pub(crate) fn load(metadata: &Metadata, snapshot: &Snapshot) -> Result<PolicyCatalog> {
    let records = snapshot
        .catalog
        .records
        .values()
        .filter(|reference| reference.identity.kind == Kind::Policy)
        .map(|reference| {
            let record: PolicyRecord =
                snapshot.read(metadata.authenticator(), reference.identity)?;
            if identity(record.uuid) != reference.identity {
                return Err(integrity_error());
            }
            Ok(record)
        })
        .collect::<Result<Vec<_>>>()?;
    PolicyCatalog::new(records).map_err(|_| integrity_error())
}

impl Store {
    /// Trusted local administration. Definitions never attach themselves.
    pub fn register_policies(&self, records: &[PolicyRecord]) -> anyhow::Result<Vec<Uuid>> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "policy registration requires local administration"
            )
        );
        let requested = PolicyCatalog::new(records.iter().cloned())?;
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let existing = load(&self.metadata, &snapshot)?;
        let mut writes = Vec::new();
        let mut combined: Vec<_> = existing.records().cloned().collect();
        for record in requested.records() {
            if snapshot.contains(identity(record.uuid))? {
                ensure!(
                    existing.get(record.uuid)? == record,
                    Error::new(
                        ErrorCode::PolicyConflict,
                        "policy UUID already has another definition"
                    )
                );
            } else {
                writes.push((identity(record.uuid), serde_json::to_value(record)?));
                combined.push(record.clone());
            }
        }
        PolicyCatalog::new(combined)?;
        if !writes.is_empty() {
            self.metadata.publish(&snapshot, writes, vec![])?;
        }
        Ok(records.iter().map(|record| record.uuid).collect())
    }

    /// Exact definition lookup, not an effective-policy projection.
    pub fn policy(&self, uuid: Uuid) -> anyhow::Result<PolicyRecord> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        if let Some(session) = &self.session {
            let registry = self.registry(session.app)?;
            self.metadata.authenticator().check_credential(
                session.app,
                &registry.registry.credential,
                session.token.as_deref(),
            )?;
            let app_readable = registry.registry.policy_refs.ids().contains(&uuid)
                && self
                    .authorize_resource(
                        &registry,
                        crate::models::claim::policy::Action::Read,
                        None,
                        None,
                    )
                    .is_ok();
            let claim_readable = registry.claims.values().any(|claim| {
                claim.policy_refs.ids().contains(&uuid)
                    && self
                        .authorize_resource(
                            &registry,
                            crate::models::claim::policy::Action::Read,
                            Some(claim.request.source_uuid),
                            Some(&claim.scope),
                        )
                        .is_ok()
            });
            let mut source_readable = false;
            for reference in snapshot
                .catalog
                .records
                .values()
                .filter(|r| r.identity.kind == Kind::Source)
            {
                let source: crate::models::source::SourceRecord =
                    snapshot.read(self.metadata.authenticator(), reference.identity)?;
                if source.policy_refs.ids().contains(&uuid)
                    && self
                        .authorize_resource(
                            &registry,
                            crate::models::claim::policy::Action::Read,
                            Some(source.uuid),
                            None,
                        )
                        .is_ok()
                {
                    source_readable = true;
                    break;
                }
            }
            ensure!(
                app_readable || claim_readable || source_readable,
                Error::new(
                    ErrorCode::AccessDenied,
                    "policy lookup requires an authorized readable association"
                )
            );
        }
        Ok(load(&self.metadata, &snapshot)?.get(uuid)?.clone())
    }
}

impl crate::core::claim::session::AppSession {
    pub fn policy(&self, uuid: Uuid) -> anyhow::Result<PolicyRecord> {
        self.store.policy(uuid)
    }
}
