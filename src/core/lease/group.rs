use crate::{
    core::{
        Store,
        claim::session::AppSession,
        policy::{
            catalog::PolicyCatalog,
            fence::PolicyFence,
            group::{GroupBoundary, strongest},
            persistence,
            resolve::{self, Inputs, Operation},
        },
        store::ResourceLease,
    },
    error::{Error, ErrorCode},
    models::{
        claim::policy::Action,
        lease::{GroupDescriptor, GroupMemberDescriptor, GroupRequest, GroupTarget},
        policy::PolicyRefs,
        store::{OutputEntry, VerificationMode},
    },
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Owns every selected output until dropped. Descriptors alone do not retain files.
pub struct GroupLease {
    pub(crate) leases: BTreeMap<Uuid, ResourceLease>,
    descriptor: GroupDescriptor,
}
impl GroupLease {
    pub fn descriptor(&self) -> &GroupDescriptor {
        &self.descriptor
    }
    pub fn member(&self, name: &str) -> Option<&GroupMemberDescriptor> {
        self.descriptor.members.get(name)
    }
    pub fn output_count(&self) -> usize {
        self.leases.len()
    }
}

impl Store {
    /// Lease prepared outputs together, without fetching, claiming or updating them.
    /// An app session is mandatory for claim targets and applies to every member.
    pub fn lease_group(
        &self,
        request: &GroupRequest,
        portable: &PolicyCatalog,
    ) -> Result<GroupLease> {
        request.validate()?;
        let started = Instant::now();
        let remaining = || Duration::from_secs(30).saturating_sub(started.elapsed());
        let (mut leases, mut descriptor, outputs, modes, fences) = {
            let _coordination = self.lock_with_timeout(remaining())?;
            let snapshot = self.metadata.snapshot()?;
            let registered = persistence::load(&self.metadata, &snapshot)?;
            let catalog = registered.with_portable(portable)?;
            let boundary = GroupBoundary::evaluate(&catalog, &request.policy_refs)?;
            let app = self.session.as_ref().map(|s| s.app);
            let registry = app
                .map(|id| {
                    self.application(id)?;
                    let registry = self.registry(id)?;
                    self.check_session(&registry)?;
                    Ok::<_, anyhow::Error>(registry)
                })
                .transpose()?;
            let empty = PolicyRefs::default();
            let app_refs = registry
                .as_ref()
                .map(|r| &r.registry.policy_refs)
                .unwrap_or(&empty);
            let persistent: Vec<_> = request
                .policy_refs
                .ids()
                .iter()
                .copied()
                .filter(|id| registered.get(*id).is_ok())
                .collect();
            let mut outputs: BTreeMap<Uuid, OutputEntry> = BTreeMap::new();
            let mut modes = BTreeMap::new();
            let mut fences = BTreeMap::new();
            let mut descriptor = GroupDescriptor {
                members: BTreeMap::new(),
            };
            // Authorize each label before deduplicating ownership: aliases never bypass authority.
            for member in &request.members {
                let (id, claim_id, scope) = match member.target {
                    GroupTarget::Output { uuid } => {
                        self.authorize_output(uuid, Action::Lease)?;
                        (uuid, None, None)
                    }
                    GroupTarget::Claim { uuid } => {
                        let registry = registry.as_ref().with_context(|| {
                            Error::new(
                                ErrorCode::AccessDenied,
                                "claim group members require an explicit application session",
                            )
                        })?;
                        let claim = registry.claims.get(&uuid).with_context(|| {
                            Error::new(ErrorCode::UnknownClaim, "claim missing from application")
                        })?;
                        self.authorize_claim(registry, claim, Action::Lease)?;
                        (claim.entry_uuid, Some(uuid), Some(&claim.scope))
                    }
                };
                let output = self.output(id)?;
                let source = self.source(output.request.source_uuid)?;
                boundary.authorize(source.uuid, scope)?;
                let plan = resolve::evaluate(
                    &catalog,
                    Inputs {
                        source: &source.policy_refs,
                        application: app_refs,
                        request: &empty,
                        pinned: None,
                        operation: Operation::Read,
                        managed_output: false,
                        update_due: false,
                    },
                )?;
                let mode = boundary.verification(plan.behavior.path_verification);
                modes
                    .entry(id)
                    .and_modify(|current| *current = strongest(*current, mode))
                    .or_insert(mode);
                if let std::collections::btree_map::Entry::Vacant(entry) = fences.entry(source.uuid)
                {
                    entry.insert(PolicyFence::capture_references(
                        self,
                        Some(source.uuid),
                        app,
                        &persistent,
                    )?);
                }
                let mut evidence = BTreeMap::new();
                for id in source.policy_refs.ids().iter().chain(app_refs.ids()) {
                    evidence.insert(*id, resolve::evidence(catalog.get(*id)?)?);
                }
                for value in &boundary.evidence {
                    evidence.insert(value.uuid, value.clone());
                }
                descriptor.members.insert(
                    member.name.clone(),
                    GroupMemberDescriptor {
                        output_uuid: id,
                        claim_uuid: claim_id,
                        directory: Default::default(),
                        relative: member.relative.clone(),
                        verification: mode,
                        policy_provenance: evidence.into_values().collect(),
                    },
                );
                outputs.insert(id, output);
            }
            // All use locks are acquired under global coordination, in stable UUID order.
            // The one deadline prevents multiplying the lock wait by the member count.
            let mut leases = BTreeMap::new();
            for (id, output) in &outputs {
                leases.insert(*id, self.metadata_lease_with_timeout(output, remaining())?);
            }
            for member in descriptor.members.values_mut() {
                member.directory = leases[&member.output_uuid]
                    .directory_view(&outputs[&member.output_uuid], member.relative.as_deref())?;
            }
            (leases, descriptor, outputs, modes, fences)
        };
        for (id, lease) in &mut leases {
            if modes[id] == VerificationMode::Full {
                lease.verify(self, &outputs[id])?;
            }
        }
        {
            let _coordination = self.lock_with_timeout(remaining())?;
            for fence in fences.values() {
                fence.check(self)?;
            }
            if let Some(session) = &self.session {
                self.check_session(&self.registry(session.app)?)?;
            }
        }
        for member in descriptor.members.values_mut() {
            member.verification = modes[&member.output_uuid];
        }
        Ok(GroupLease { leases, descriptor })
    }
}
impl AppSession {
    pub fn lease_group(
        &self,
        request: &GroupRequest,
        portable: &PolicyCatalog,
    ) -> Result<GroupLease> {
        self.store.lease_group(request, portable)
    }
}

#[cfg(test)]
mod tests;
