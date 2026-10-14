use super::*;
use crate::{
    core::policy::{
        group::GroupBoundary,
        persistence,
        resolve::{self, Inputs, Operation},
    },
    models::{
        group::{AcquireMember, Target, Witness},
        policy::record::CacheMode,
        store::{OutputPlan, Selection},
    },
};
use anyhow::Context;
use std::collections::BTreeMap;

impl Store {
    pub(super) fn check_group_owner(&self, owner: Option<Uuid>) -> Result<()> {
        if let Some(session) = &self.session {
            ensure!(
                owner == Some(session.app),
                Error::new(ErrorCode::AccessDenied, "group owner mismatch")
            );
            self.application(session.app)?;
            self.check_session(&self.registry(session.app)?)?;
        }
        Ok(())
    }
    pub(super) fn authorize_group(&self, group: &GroupRecord, action: Action) -> Result<()> {
        self.check_group_owner(group.owner)?;
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let boundary = GroupBoundary::evaluate(&catalog, &group.definition.policy_refs)?;
        for witness in group.witnesses.values() {
            if let Some(session) = &self.session {
                self.authorize_resource(
                    &self.registry(session.app)?,
                    action,
                    Some(witness.source_uuid),
                    witness.scope.as_ref(),
                )?;
            }
            boundary.authorize_action(action, witness.source_uuid, witness.scope.as_ref())?;
        }
        Ok(())
    }
    /// Pure preflight: all references and authorities are checked before provider work.
    pub(super) fn validate_group(
        &self,
        owner: Option<Uuid>,
        definition: &GroupDefinition,
    ) -> Result<BTreeMap<String, Witness>> {
        self.check_group_owner(owner)?;
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let boundary = GroupBoundary::evaluate(&catalog, &definition.policy_refs)?;
        let mut witnesses = BTreeMap::new();
        for member in &definition.members {
            let (source_uuid, scope, action) = match &member.target {
                Target::Acquire(input) => {
                    ensure!(
                        input.claim.is_some() == owner.is_some(),
                        Error::new(
                            ErrorCode::InvalidRequest,
                            "app acquisition requires explicit claim scope; local acquisition must omit claim"
                        )
                    );
                    self.group_acquisition_plan(owner, input)?;
                    (
                        input.source_uuid,
                        input.claim.as_ref().map(|c| c.scope.clone()),
                        Action::Acquire,
                    )
                }
                Target::Claim { uuid } => {
                    let app = owner.context(Error::new(
                        ErrorCode::AccessDenied,
                        "claim target requires an explicit app owner",
                    ))?;
                    let registry = self.registry(app)?;
                    let claim = registry
                        .claims
                        .get(uuid)
                        .context(Error::new(ErrorCode::UnknownClaim, "group claim missing"))?;
                    if self.session.is_some() {
                        self.authorize_claim(&registry, claim, Action::Read)?;
                        self.authorize_claim(&registry, claim, Action::Lease)?;
                    }
                    (
                        claim.request.source_uuid,
                        Some(claim.scope.clone()),
                        Action::Lease,
                    )
                }
                Target::Output { uuid } => {
                    let output = self.output(*uuid)?;
                    let directory = self.metadata_path(&output)?;
                    crate::core::store::access::directory_view(
                        &directory,
                        &output,
                        member.relative.as_deref(),
                    )?;
                    let scope = if let Some(app) = owner {
                        let registry = self.registry(app)?;
                        let claim = registry
                            .claims
                            .values()
                            .find(|c| {
                                c.entry_uuid == *uuid
                                    && (self.session.is_none()
                                        || (self
                                            .authorize_claim(&registry, c, Action::Read)
                                            .is_ok()
                                            && self
                                                .authorize_claim(&registry, c, Action::Lease)
                                                .is_ok()))
                            })
                            .context(Error::new(
                                ErrorCode::AccessDenied,
                                "group output requires an authorized owned claim",
                            ))?;
                        Some(claim.scope.clone())
                    } else {
                        None
                    };
                    (output.request.source_uuid, scope, Action::Lease)
                }
            };
            boundary.authorize_action(action, source_uuid, scope.as_ref())?;
            witnesses.insert(member.name.clone(), Witness { source_uuid, scope });
        }
        Ok(witnesses)
    }
    pub(super) fn group_acquisition_plan(
        &self,
        owner: Option<Uuid>,
        input: &AcquireMember,
    ) -> Result<OutputPlan> {
        let source = self.source(input.source_uuid)?;
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let app_refs = owner
            .map(|id| self.registry(id).map(|r| r.registry.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let plan = resolve::evaluate(
            &catalog,
            Inputs {
                source: &source.policy_refs,
                application: &app_refs,
                request: &input.policy_refs,
                pinned: None,
                operation: Operation::Compose,
                managed_output: true,
                update_due: false,
            },
        )?;
        ensure!(
            plan.behavior.cache == CacheMode::Cached,
            Error::new(
                ErrorCode::PolicyConflict,
                "persistent groups require cached retrieval"
            )
        );
        let scope = input.claim.as_ref().map(|c| &c.scope);
        if let Some(app) = self.session.as_ref().map(|s| s.app) {
            self.authorize_resource(
                &self.registry(app)?,
                Action::Acquire,
                Some(source.uuid),
                scope,
            )?;
        }
        // Local requests still obey explicitly attached source restrictions.
        resolve::authorize(&plan, Action::Acquire, Some(source.uuid), scope)?;
        let selection = Selection {
            platform: input.selection.platform.clone(),
            filters: resolve::filters(&catalog, &input.selection.filter_refs)?,
            format: input.selection.format,
            filename: input.selection.filename.clone(),
            folder: input.selection.folder.clone(),
            bin: input.selection.bin.clone(),
            sha256: input.selection.sha256.clone(),
            selection_policy: plan.behavior.selection.clone(),
        };
        let adopted = crate::models::claim::adopted::AdoptedPolicies::from_plan(
            &plan,
            input.selection.filter_refs.clone(),
        );
        let request = OutputPlan {
            source_uuid: source.uuid,
            selector: input.selector.clone(),
            selection,
            policies: crate::core::policy::adopt::effective(&plan)?.output,
            adopted: Some(Box::new(adopted)),
        };
        crate::core::policy::validate(&request)?;
        // Validate source/selector and cache combinations without retrieval side effects.
        let (retrieval, acquisition) = self.composition_requests(&request, &Default::default())?;
        crate::core::policy::composition::validate_composition(&retrieval, &acquisition, false)?;
        crate::core::source::retrieve::validate(&source.source, &retrieval)?;
        Ok(request)
    }
}
