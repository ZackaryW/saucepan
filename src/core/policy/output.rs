//! Plan managed output intent before provider work or lifecycle reservation.
use super::{
    adopt, claims,
    fence::PolicyFence,
    persistence,
    resolve::{self, Inputs, Operation},
};
use crate::{
    core::Store,
    error::{Error, ErrorCode},
    models::{
        claim::{adopted::AdoptedPolicies, policy::Action},
        policy::{PolicyRefs, record::CacheMode},
        store::{OutputPlan, Request, Selection},
    },
};
use anyhow::{Result, ensure};

impl Store {
    pub(crate) fn plan_output(&self, input: &Request) -> Result<(Self, OutputPlan)> {
        let _lock = self.lock()?;
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "use scoped acquisition for application requests"
            )
        );
        let source = self.source(input.source_uuid)?;
        let snapshot = self.metadata.snapshot()?;
        let catalog = persistence::load(&self.metadata, &snapshot)?;
        let empty = PolicyRefs::default();
        let plan = resolve::evaluate(
            &catalog,
            Inputs {
                source: &source.policy_refs,
                application: &empty,
                request: &input.policy_refs,
                pinned: None,
                operation: Operation::Compose,
                managed_output: true,
                update_due: false,
            },
        )?;
        resolve::authorize(&plan, Action::Acquire, Some(input.source_uuid), None)?;
        ensure!(
            plan.behavior.cache == CacheMode::Cached,
            Error::new(
                ErrorCode::PolicyConflict,
                "managed acquisition requires cached retrieval; use composition for temporary outputs"
            )
        );
        let adopted = AdoptedPolicies::from_plan(&plan, input.selection.filter_refs.clone());
        claims::validate_adopted(&catalog, &adopted)?;
        let request = OutputPlan {
            adopted: Some(Box::new(adopted)),
            source_uuid: input.source_uuid,
            selector: input.selector.clone(),
            selection: Selection {
                platform: input.selection.platform.clone(),
                filters: resolve::filters(&catalog, &input.selection.filter_refs)?,
                selection_policy: plan.behavior.selection.clone(),
                format: input.selection.format,
                filename: input.selection.filename.clone(),
                folder: input.selection.folder.clone(),
                bin: input.selection.bin.clone(),
                sha256: input.selection.sha256.clone(),
            },
            policies: adopt::effective(&plan)?.output,
        };
        super::validate(&request)?;
        let mut execution = self.clone();
        let references: Vec<_> = input
            .policy_refs
            .ids()
            .iter()
            .chain(input.selection.filter_refs.ids())
            .copied()
            .collect();
        execution.policy_fence = Some(
            PolicyFence::capture_references(self, Some(source.uuid), None, &references)?
                .with_authority(plan, None),
        );
        Ok((execution, request))
    }
}
