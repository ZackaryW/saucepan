//! Resolve a scoped intent before using the existing identity/publication engine.
use crate::{
    core::{
        Store,
        claim::session::AppSession,
        policy::{
            adopt, claims, persistence,
            resolve::{self, Inputs, Operation},
        },
        source::AcquireOptions,
    },
    error::{Error, ErrorCode},
    models::{
        claim::{
            Claimed, CompiledEnsure, adopted::AdoptedPolicies, policy::Action, request::Ensure,
        },
        policy::{PolicyRefs, record::CacheMode},
        store::{Request, Selection},
    },
};
use anyhow::{Result, ensure};
use uuid::Uuid;

impl Store {
    pub fn ensure_claim(
        &self,
        app: Uuid,
        input: &Ensure,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.ensure_referenced(app, input, options)
    }
    pub fn ensure_referenced(
        &self,
        app: Uuid,
        input: &Ensure,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        let (compiled, fence) = {
            let _lock = self.lock()?;
            let registry = self.registry(app)?;
            self.authorize_resource(
                &registry,
                Action::Acquire,
                Some(input.source_uuid),
                Some(&input.scope),
            )?;
            let source = self.source(input.source_uuid)?;
            let snapshot = self.metadata.snapshot()?;
            let catalog = persistence::load(&self.metadata, &snapshot)?;
            let selection = Selection {
                platform: input.selection.platform.clone(),
                filters: resolve::filters(&catalog, &input.selection.filter_refs)?,
                format: input.selection.format,
                filename: input.selection.filename.clone(),
                folder: input.selection.folder.clone(),
                bin: input.selection.bin.clone(),
                sha256: input.selection.sha256.clone(),
                ..Default::default()
            };
            let request = Request {
                adopted: None,
                source_uuid: input.source_uuid,
                selector: input.selector.clone(),
                selection,
                policies: Default::default(),
            };
            let existing =
                super::matching::select_identity(self, &registry, &input.scope, &request)?;
            let pinned = existing
                .as_ref()
                .and_then(|m| m.claim.request.adopted.as_deref())
                .map(AdoptedPolicies::pinned);
            let plan = resolve::evaluate(
                &catalog,
                Inputs {
                    source: &source.policy_refs,
                    application: &registry.registry.policy_refs,
                    request: &input.policy_refs,
                    pinned: pinned.as_ref(),
                    operation: Operation::Compose,
                    managed_output: true,
                    update_due: false,
                },
            )?;
            resolve::authorize(
                &plan,
                Action::Acquire,
                Some(input.source_uuid),
                Some(&input.scope),
            )?;
            ensure!(
                plan.behavior.cache == CacheMode::Cached,
                Error::new(
                    ErrorCode::PolicyConflict,
                    "persistent claims require cached retrieval"
                )
            );
            let mut adopted =
                AdoptedPolicies::from_plan(&plan, input.selection.filter_refs.clone());
            if let Some(pinned) = &pinned {
                ensure!(
                    pinned.behavior == plan.behavior,
                    Error::new(
                        ErrorCode::ClaimOverlap,
                        "claim behavior differs; use explicit policy adoption"
                    )
                );
                // Equivalent UUIDs never silently replace an existing association.
                adopted = *existing
                    .as_ref()
                    .unwrap()
                    .claim
                    .request
                    .adopted
                    .clone()
                    .unwrap();
            }
            claims::validate_adopted(&catalog, &adopted)?;
            let effective = adopt::effective(&plan)?;
            let mut selection = request.selection;
            selection.selection_policy = plan.behavior.selection.clone();
            // Capture the same attachments used during planning, before releasing
            // coordination for provider work.
            let fence = crate::core::policy::fence::PolicyFence::capture(
                self,
                Some(source.uuid),
                Some(app),
            )?;
            (
                CompiledEnsure {
                    adopted: Some(Box::new(adopted)),
                    scope: input.scope.clone(),
                    source_uuid: input.source_uuid,
                    selector: input.selector.clone(),
                    selection,
                    context: input.context.clone(),
                    overrides: adopt::pinned(&effective),
                },
                fence,
            )
        };
        let mut execution = self.clone();
        execution.policy_fence = Some(fence);
        execution.ensure_compiled(app, &compiled, options)
    }
    pub(crate) fn adopted_refs(&self, adopted: Option<&AdoptedPolicies>) -> Result<PolicyRefs> {
        let Some(adopted) = adopted else {
            return Ok(Default::default());
        };
        let snapshot = self.metadata.snapshot()?;
        claims::behavior_refs(&persistence::load(&self.metadata, &snapshot)?, adopted)
    }
}
impl AppSession {
    pub fn ensure_referenced(&self, input: &Ensure, options: &AcquireOptions) -> Result<Claimed> {
        self.store.ensure_referenced(self.app, input, options)
    }
}
