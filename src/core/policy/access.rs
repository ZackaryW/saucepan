//! Resolve path verification independently of retrieval and copied-output policy.
use super::{
    catalog::PolicyCatalog,
    fence::PolicyFence,
    persistence,
    resolve::{self, Inputs, Operation},
};
use crate::{
    core::{Store, claim::session::AppSession, store::ResourceLease},
    error::{Error, ErrorCode},
    models::{
        claim::CoveredView,
        policy::{PolicyRefs, requests::PathAccess},
        store::{ResolvedResource, VerificationMode},
    },
};
use anyhow::{Context, Result};
use uuid::Uuid;

impl Store {
    pub fn resolve(&self, entry: Uuid, mode: VerificationMode) -> Result<ResolvedResource> {
        let (input, catalog) = path_input(vec![], Some(mode))?;
        self.resolve_referenced(entry, &input, &catalog)
    }

    pub fn lease(&self, entry: Uuid, mode: VerificationMode) -> Result<ResourceLease> {
        let (input, catalog) = path_input(vec![], Some(mode))?;
        self.lease_referenced(entry, &input, &catalog)
    }

    pub fn lease_view(&self, view: &CoveredView, mode: VerificationMode) -> Result<ResourceLease> {
        let (input, catalog) = path_input(vec![], Some(mode))?;
        self.lease_view_referenced(view, &input, &catalog)
    }

    pub fn lease_claim(
        &self,
        app: Uuid,
        claim: Uuid,
        mode: VerificationMode,
    ) -> Result<ResourceLease> {
        let (input, portable) = path_input(vec![], Some(mode))?;
        self.lease_claim_referenced(app, claim, &input, &portable)
    }

    pub fn lease_claim_referenced(
        &self,
        app: Uuid,
        id: Uuid,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<ResourceLease> {
        let (execution, mode) = {
            let _lock = self.lock()?;
            let registry = self.registry(app)?;
            self.check_session(&registry)?;
            let claim = registry
                .claims
                .get(&id)
                .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
            self.authorize_claim(
                &registry,
                claim,
                crate::models::claim::policy::Action::Lease,
            )?;
            self.path_context(claim.request.source_uuid, Some(app), input, portable)?
        };
        execution.check_policy_fence()?;
        let lease = execution.lease_claim_planned(app, id, mode)?;
        execution.check_policy_fence()?;
        Ok(lease)
    }

    fn path_context(
        &self,
        source: Uuid,
        app: Option<Uuid>,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<(Self, VerificationMode)> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let registered = persistence::load(&self.metadata, &snapshot)?;
        let catalog = registered.with_portable(portable)?;
        let source_refs = self.source(source)?.policy_refs;
        let app_refs = app
            .map(|id| self.registry(id).map(|r| r.registry.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let plan = resolve::evaluate(
            &catalog,
            Inputs {
                source: &source_refs,
                application: &app_refs,
                request: &input.policy_refs,
                pinned: None,
                operation: Operation::Read,
                managed_output: false,
                update_due: false,
            },
        )?;
        let persistent_refs: Vec<_> = input
            .policy_refs
            .ids()
            .iter()
            .copied()
            .filter(|id| registered.get(*id).is_ok())
            .collect();
        let mut execution = self.clone();
        execution.policy_fence = Some(PolicyFence::capture_references(
            self,
            Some(source),
            app,
            &persistent_refs,
        )?);
        Ok((execution, plan.behavior.path_verification))
    }

    pub fn resolve_referenced(
        &self,
        entry: Uuid,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<ResolvedResource> {
        let (execution, mode) = {
            let _lock = self.lock()?;
            self.authorize_output(entry, crate::models::claim::policy::Action::Lease)?;
            self.path_context(
                self.output(entry)?.request.source_uuid,
                self.session.as_ref().map(|s| s.app),
                input,
                portable,
            )?
        };
        execution.check_policy_fence()?;
        let result = execution.resolve_planned(entry, mode)?;
        execution.check_policy_fence()?;
        Ok(result)
    }

    pub fn lease_referenced(
        &self,
        entry: Uuid,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<ResourceLease> {
        let (execution, mode) = {
            let _lock = self.lock()?;
            self.authorize_output(entry, crate::models::claim::policy::Action::Lease)?;
            self.path_context(
                self.output(entry)?.request.source_uuid,
                self.session.as_ref().map(|s| s.app),
                input,
                portable,
            )?
        };
        execution.check_policy_fence()?;
        let lease = execution.lease_planned(entry, mode)?;
        execution.check_policy_fence()?;
        Ok(lease)
    }

    pub fn lease_view_referenced(
        &self,
        view: &CoveredView,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<ResourceLease> {
        view.validate()?;
        let (execution, mode) = {
            let _lock = self.lock()?;
            let registry = self.registry(view.app)?;
            self.check_session(&registry)?;
            let claim = registry
                .claims
                .get(&view.claim)
                .with_context(|| Error::new(ErrorCode::UnknownClaim, "claim missing"))?;
            self.authorize_claim(
                &registry,
                claim,
                crate::models::claim::policy::Action::Lease,
            )?;
            self.path_context(claim.request.source_uuid, Some(view.app), input, portable)?
        };
        execution.check_policy_fence()?;
        let lease = execution.lease_view_planned(view, mode)?;
        execution.check_policy_fence()?;
        Ok(lease)
    }
}

impl AppSession {
    pub fn lease_view_referenced(
        &self,
        view: &CoveredView,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<ResourceLease> {
        anyhow::ensure!(
            view.app == self.app,
            Error::new(
                ErrorCode::AccessDenied,
                "view belongs to another application"
            )
        );
        self.store.lease_view_referenced(view, input, portable)
    }
    pub fn lease_output_referenced(
        &self,
        entry: Uuid,
        input: &PathAccess,
        portable: &PolicyCatalog,
    ) -> Result<ResourceLease> {
        self.store.lease_referenced(entry, input, portable)
    }
}

/// Native and CLI convenience values enter the same evaluator as referenced definitions.
/// A caller cannot select both an explicit mode and policy references.
pub(crate) fn path_input(
    refs: Vec<Uuid>,
    mode: Option<VerificationMode>,
) -> Result<(PathAccess, PolicyCatalog)> {
    anyhow::ensure!(
        mode.is_none() || refs.is_empty(),
        Error::new(
            ErrorCode::InvalidRequest,
            "verification and policy references cannot be combined"
        )
    );
    if let Some(mode) = mode {
        let record = crate::models::policy::PolicyRecord {
            uuid: Uuid::new_v4(),
            definition: crate::models::policy::PolicyDefinition::VerificationPolicy(
                crate::models::policy::record::VerificationPolicy::Path { mode },
            ),
        };
        Ok((
            PathAccess {
                policy_refs: vec![record.uuid].try_into()?,
            },
            PolicyCatalog::new([record])?,
        ))
    } else {
        Ok((
            PathAccess {
                policy_refs: PolicyRefs::try_from(refs)?,
            },
            PolicyCatalog::default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        source::Source,
        store::{Request, Selector},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn path_verification_fences_adoption_but_not_unrelated_registration() {
        for adopt in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let file = tmp.path().join("input");
            std::fs::write(&file, b"content").unwrap();
            let mut store = Store::create_fixture(&tmp.path().join("root")).unwrap();
            let source = store.register_source(Source::Local { path: file }).unwrap();
            let output = store
                .acquire(&Request {
                    source_uuid: source.uuid,
                    selector: Selector::Local,
                    selection: Default::default(),
                    policy_refs: Default::default(),
                })
                .unwrap();
            let (input, portable) = path_input(vec![], Some(VerificationMode::Full)).unwrap();
            let record = crate::models::policy::PolicyRecord {
                uuid: Uuid::new_v4(),
                definition: crate::models::policy::PolicyDefinition::VerificationPolicy(
                    crate::models::policy::record::VerificationPolicy::Path {
                        mode: VerificationMode::Metadata,
                    },
                ),
            };
            let editor = store.clone();
            let once = AtomicBool::new(false);
            store.access_observer = Some(Arc::new(move |event| {
                if matches!(event, crate::utils::tree::IoEvent::Open)
                    && !once.swap(true, Ordering::SeqCst)
                {
                    editor
                        .register_policies(std::slice::from_ref(&record))
                        .unwrap();
                    if adopt {
                        editor
                            .adopt_source_policies(
                                source.uuid,
                                0,
                                vec![record.uuid].try_into().unwrap(),
                            )
                            .unwrap();
                    }
                }
            }));
            let result = store.lease_referenced(output.entry.uuid, &input, &portable);
            if adopt {
                assert_eq!(
                    Error::from_anyhow(&result.err().unwrap()).code,
                    ErrorCode::RevisionConflict
                );
            } else {
                assert_eq!(
                    result.unwrap().resource().verification,
                    VerificationMode::Full
                );
            }
        }
    }
}
