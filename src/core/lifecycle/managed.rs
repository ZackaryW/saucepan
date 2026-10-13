use super::*;
use crate::{
    core::{shims, source::AcquireOptions, store::ResourceLease},
    models::{
        index::{
            Application, Binding, Claim, OwnedShim,
            lifecycle::{Installation, Probe, ShimChange},
        },
        policy::Protection,
        store::{OutputPlan, VerificationMode},
    },
    utils::{fs::atomic_write, tree},
};
use std::path::Path;

fn executable(request: &OutputPlan) -> Result<&str> {
    ensure!(
        request.policies.protection == Some(Protection::UseableApp),
        "managed application requires useable_app"
    );
    ensure!(
        request.selection.bin.len() == 1,
        "managed application requires one executable"
    );
    Ok(&request.selection.bin[0])
}
impl Store {
    pub fn install_application(
        &self,
        input: &Installation,
        built_shim: &Path,
        options: &AcquireOptions,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        let (execution, request) = self.plan_output(&input.request)?;
        execution.install_planned(input, &request, built_shim, options, operation)
    }
    fn install_planned(
        &self,
        input: &Installation,
        request: &OutputPlan,
        built_shim: &Path,
        options: &AcquireOptions,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        ensure!(!input.name.trim().is_empty(), "empty app name");
        let binary = executable(request)?.to_owned();
        let sidecar = shims::sidecar(&input.shim)?;
        let digest = shims::digest(built_shim)?;
        let (mut op, _guard) = self.begin_operation(
            operation.unwrap_or_else(Uuid::new_v4),
            None,
            Action::Install {
                scope: input.scope.clone(),
                name: input.name.clone(),
                request: Box::new(request.clone()),
                probe: input.probe.clone(),
                shim: input.shim.clone(),
                digest: digest.clone(),
                saucepan: input.saucepan,
            },
        )?;
        if op.status.terminal() {
            return Ok(op);
        }
        if op.status != Phase::Preparing {
            self.resume_application(&mut op)?;
            return Ok(op);
        }
        let prepared = (|| -> Result<ResourceLease> {
            let desired_shim = OwnedShim {
                name: input.shim.clone(),
                sidecar,
                digest,
                config_digest: tree::digest(
                    shims::config_bytes(op.app, input.saucepan)?.as_slice(),
                )?,
                saucepan: input.saucepan,
            };
            {
                let _lock = self.lock()?;
                self.reserve_shim_locked(&op, &desired_shim, None)?;
                op.shims = vec![ShimChange {
                    previous: None,
                    desired: desired_shim.clone(),
                }];
                self.save_operation(&op)?;
            }
            let staged = beneath(&self.operation_dir(op.operation)?, "launcher")?;
            atomic_write(&staged, |f| {
                std::io::copy(&mut fs::File::open(built_shim)?, f)?;
                Ok(())
            })?;
            ensure!(
                shims::digest(&staged)? == desired_shim.digest,
                "built shim changed during staging"
            );
            let (candidate, _guard) = self.acquire_candidate(request, options)?;
            let lease = self.validate_candidate(candidate.entry.uuid, &binary, &input.probe)?;
            let claim = Claim {
                policy_refs: self.adopted_refs(request.adopted.as_deref())?,
                scope: input.scope.clone(),
                effective: crate::core::policy::adopt::effective(
                    &request.adopted.as_ref().expect("planned output").pinned(),
                )?,
                context: Default::default(),
                uuid: op.claim,
                request: request.clone(),
                entry_uuid: candidate.entry.uuid,
            };
            let mut registry = Self::new_registry(op.app, 1)?;
            registry.claims.insert(claim.uuid, claim);
            op.desired_registry = Some(registry);
            op.candidate = Some(candidate.entry.uuid);
            op.desired = Some(Application {
                uuid: op.app,
                name: input.name.clone(),
                settings: Default::default(),
                active: Some(Binding {
                    entry_uuid: candidate.entry.uuid,
                    executable: binary,
                }),
                state: AppState::Ready,
                revision: 1,
                shims: vec![desired_shim],
            });
            let _lock = self.lock()?;
            self.check_operation_policy(&op)?;
            op.status = Phase::CommitIntent;
            op.pending.clear();
            self.save_operation(&op)?;
            Ok(lease)
        })();
        match prepared {
            Ok(_lease) => self.finish_application(&mut op)?,
            Err(error) => self.fail_preparation(&mut op, error)?,
        }
        Ok(op)
    }
    pub fn update_application(
        &self,
        app: Uuid,
        claim: Uuid,
        probe: &Probe,
        rollback: Option<Uuid>,
        options: &AcquireOptions,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        if self.policy_fence.is_none() {
            let _lock = self.lock()?;
            let registry = self.registry(app)?;
            self.check_session(&registry)?;
            let selected = registry.claims.get(&claim).ok_or_else(|| {
                crate::error::Error::new(crate::error::ErrorCode::UnknownClaim, "claim missing")
            })?;
            let execution = self.with_policy_fence(selected.request.source_uuid, Some(app))?;
            drop(_lock);
            return execution.update_application(app, claim, probe, rollback, options, operation);
        }
        let (mut op, _guard) = self.begin_operation(
            operation.unwrap_or_else(Uuid::new_v4),
            Some(app),
            Action::Update {
                claim,
                probe: probe.clone(),
                rollback,
            },
        )?;
        if op.status.terminal() {
            return Ok(op);
        }
        if op.status != Phase::Preparing {
            self.resume_application(&mut op)?;
            return Ok(op);
        }
        let prepared = (|| -> Result<ResourceLease> {
            let mut desired = {
                let _lock = self.lock()?;
                let current = self.application(app)?;
                ensure!(
                    Some(current.revision) == op.expected_revision,
                    crate::error::Error::new(
                        crate::error::ErrorCode::RevisionConflict,
                        "application revision conflict; retry with a new operation"
                    )
                );
                current
            };
            let mut registry = self.registry(app)?;
            let request = registry
                .claims
                .get(&claim)
                .context("claim missing")?
                .request
                .clone();
            let binary = executable(&request)?.to_owned();
            let (entry, _guard) = if let Some(entry) = rollback {
                let _lock = self.lock()?;
                let output = self.output(entry)?;
                ensure!(
                    output.item_key
                        == crate::core::store::json_digest(&(
                            request.source_uuid,
                            &request.selector,
                            &request.selection
                        ))?,
                    "rollback target belongs to another item"
                );
                // Authorization belongs to the claim being transitioned. A
                // historical candidate is intentionally not its current output.
                let guard = self.metadata_lease_locked(&output)?;
                (entry, guard)
            } else {
                let (candidate, guard) = self.acquire_candidate(&request, options)?;
                (candidate.entry.uuid, guard)
            };
            let lease = self.validate_candidate(entry, &binary, probe)?;
            registry
                .claims
                .get_mut(&claim)
                .context("claim missing")?
                .entry_uuid = entry;
            desired.active = Some(Binding {
                entry_uuid: entry,
                executable: binary,
            });
            desired.bump()?;
            op.claim = claim;
            op.candidate = Some(entry);
            registry.registry.revision = desired.revision;
            op.desired_registry = Some(registry);
            op.desired = Some(desired);
            let _lock = self.lock()?;
            self.check_operation_policy(&op)?;
            ensure!(
                Some(self.application(app)?.revision) == op.expected_revision,
                crate::error::Error::new(
                    crate::error::ErrorCode::RevisionConflict,
                    "application revision conflict; retry with a new operation"
                )
            );
            op.status = Phase::CommitIntent;
            op.pending.clear();
            self.save_operation(&op)?;
            Ok(lease)
        })();
        match prepared {
            Ok(_lease) => self.finish_application(&mut op)?,
            Err(error) => self.fail_preparation(&mut op, error)?,
        }
        Ok(op)
    }
    fn fail_preparation(&self, op: &mut Operation, error: anyhow::Error) -> Result<()> {
        let _lock = self.lock()?;
        op.reason = Some(crate::error::Error::from_anyhow(&error));
        op.status = if op.reason.as_ref().unwrap().code == crate::error::ErrorCode::RevisionConflict
        {
            Phase::Conflict
        } else {
            Phase::Failed
        };
        op.pending = vec![op.reason.as_ref().unwrap().message.clone()];
        op.candidate = None;
        self.save_operation(op)
    }
    pub(crate) fn resume_application(&self, op: &mut Operation) -> Result<()> {
        if op.status == Phase::CleanupPending {
            return self.cleanup_failed_install(op);
        }
        let entry = op.candidate.context("application intent lacks candidate")?;
        let mut maintenance = self.clone();
        maintenance.session = None;
        // This private path consumes an authenticated committed receipt; it does
        // not grant app callers unrestricted direct-output access.
        match maintenance.lease(entry, VerificationMode::Full) {
            Ok(_lease) => self.finish_application(op),
            Err(error) => {
                {
                    let _lock = self.lock()?;
                    op.candidate = None;
                    op.reason = Some(crate::error::Error::from_anyhow(&error));
                    op.pending = vec!["candidate unavailable".into()];
                    op.status = if matches!(op.action, Action::Install { .. }) {
                        Phase::CleanupPending
                    } else {
                        Phase::Failed
                    };
                    self.save_operation(op)?;
                }
                if op.status == Phase::CleanupPending {
                    self.cleanup_failed_install(op)?;
                }
                Ok(())
            }
        }
    }
    fn finish_application(&self, op: &mut Operation) -> Result<()> {
        let result = (|| -> Result<()> {
            let _shims = op
                .shims
                .iter()
                .map(|s| shims::shim_lock(&self.root, &s.desired.name, LockMode::Exclusive))
                .collect::<Result<Vec<_>>>()?;
            let _lock = self.lock()?;
            self.check_operation_policy(op)?;
            let desired = op.desired.as_ref().context("application intent missing")?;
            if matches!(op.action, Action::Install { .. }) {
                ensure!(
                    !beneath(&self.root, &format!("index/{}", op.app))?.exists(),
                    "application already exists"
                );
            } else {
                let current = self.application(op.app)?;
                if Some(current.revision) != op.expected_revision {
                    op.status = Phase::Conflict;
                    op.reason = Some(crate::error::Error::new(
                        crate::error::ErrorCode::RevisionConflict,
                        "application revision conflict",
                    ));
                    op.pending =
                        vec!["application revision conflict; retry with a new operation".into()];
                    op.candidate = None;
                    return self.save_operation(op);
                }
            }
            for change in &op.shims {
                self.publish_shim_locked(op, change)?;
            }
            let record = Record::Application(desired.clone());
            op.status = Phase::Complete;
            op.reason = None;
            op.changed = true;
            op.pending.clear();
            self.commit_operation(op, vec![record])
        })();
        if let Err(error) = result {
            let _lock = self.lock()?;
            *op = self.operation(op.operation)?;
            if op.status == Phase::Complete {
                return Ok(());
            }
            if matches!(op.action, Action::Update { .. }) && self.application(op.app).is_err() {
                op.status = Phase::Conflict;
                op.reason = Some(crate::error::Error::new(
                    crate::error::ErrorCode::RevisionConflict,
                    "application revision conflict",
                ));
                op.candidate = None;
            }
            if op.status != Phase::Conflict {
                op.reason = Some(crate::error::Error::from_anyhow(&error));
            }
            op.pending = vec![op.reason.as_ref().unwrap().message.clone()];
            self.save_operation(op)?;
        }
        Ok(())
    }
    fn check_operation_policy(&self, op: &Operation) -> Result<()> {
        self.check_policy_fence()?;
        if let Some(observed) = &op.source_policy {
            ensure!(
                self.source(observed.source_uuid)?.policy_revision == observed.revision,
                crate::error::Error::new(
                    crate::error::ErrorCode::RevisionConflict,
                    "source policy changed during application operation"
                )
            );
        }
        Ok(())
    }
    fn cleanup_failed_install(&self, op: &mut Operation) -> Result<()> {
        let mut remaining = Vec::new();
        for change in &op.shims {
            let result = (|| -> Result<()> {
                let _shim =
                    shims::shim_lock(&self.root, &change.desired.name, LockMode::Exclusive)?;
                let _lock = self.lock()?;
                self.remove_owned_shim_locked(&change.desired)
            })();
            if let Err(error) = result {
                remaining.push(format!("{}: {error:#}", change.desired.name));
            }
        }
        let _lock = self.lock()?;
        if remaining.is_empty() {
            op.status = Phase::Failed;
        } else {
            op.pending.extend(remaining);
        }
        self.save_operation(op)
    }
}
