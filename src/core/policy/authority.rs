//! Current registered restrictions shared by discovery, direct access and replay.
use crate::{
    core::Store,
    models::{
        claim::{Scope, policy::Action},
        policy::PolicyRefs,
    },
};
use anyhow::Result;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) enum Projection {
    Resource,
    SourceDiscovery,
    ActionOnly,
}
impl Store {
    pub(crate) fn authorize_policy_refs(
        &self,
        app: &PolicyRefs,
        source: Option<Uuid>,
        scope: Option<&Scope>,
        action: Action,
        projection: Projection,
    ) -> Result<()> {
        let snapshot = self.metadata.snapshot()?;
        let catalog = super::persistence::load(&self.metadata, &snapshot)?;
        let source_refs = source
            .map(|id| self.source(id).map(|record| record.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let empty = PolicyRefs::default();
        let mut plan = super::resolve::evaluate(
            &catalog,
            super::resolve::Inputs {
                source: &source_refs,
                application: app,
                request: &empty,
                pinned: None,
                operation: super::resolve::Operation::Read,
                managed_output: false,
                update_due: false,
            },
        )?;
        for rule in &mut plan.access {
            if matches!(
                projection,
                Projection::SourceDiscovery | Projection::ActionOnly
            ) {
                rule.scopes = None;
            }
            if matches!(projection, Projection::ActionOnly) {
                rule.sources = None;
            }
        }
        super::resolve::authorize(&plan, action, source, scope)?;
        Ok(())
    }
}
