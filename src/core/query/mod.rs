//! Application-scoped claim discovery. Results carry no retained resource lifetime.
mod matching;
pub mod model;
mod source;
use crate::core::Store;
use anyhow::Result;
pub use model::{
    ClaimQuery, Condition, DiscoveryQuery, Operator, QueryResult, SourceQuery, SourceQueryResult,
    SourceType, SourceView,
};

impl Store {
    /// Read one coherent filtered application view, in claim UUID order.
    /// Call `lease` on a returned output, or `lease_claim` for the current claim
    /// selection, before consuming resources that must survive reclaim.
    pub fn query_claims(&self, query: &ClaimQuery) -> Result<QueryResult> {
        let conditions = matching::CompiledConditions::compile(&query.conditions)?;
        let _lock = self.lock()?;
        let application = self.application(query.app)?;
        let revision = application.revision;
        let claims = self.claim_views_locked(application, |claim| {
            query
                .scope
                .as_ref()
                .is_none_or(|scope| scope == &claim.scope)
                && conditions.matches(&claim.context)
        })?;
        Ok(QueryResult {
            app: query.app,
            revision,
            claims,
        })
    }
    /// Anonymous discovery only exposes explicitly public scopes. This returns
    /// metadata, never a claim grant or a retained output lifetime.
    pub fn discover_claims(&self, query: &DiscoveryQuery) -> Result<Vec<QueryResult>> {
        let conditions = matching::CompiledConditions::compile(&query.conditions)?;
        let _lock = self.lock()?;
        let mut found = Vec::new();
        for record in self.records()? {
            let crate::models::index::Record::Application(app) = record else {
                continue;
            };
            if app.state != crate::models::index::AppState::Ready {
                continue;
            }
            let registry = self.registry(app.uuid)?;
            let mut claims = Vec::new();
            for claim in registry.claims.values() {
                if registry.publicly_visible(claim)
                    && self
                        .authorize_policy_refs(
                            &registry.registry.policy_refs,
                            Some(claim.request.source_uuid),
                            Some(&claim.scope),
                            crate::models::claim::policy::Action::Discover,
                            crate::core::policy::authority::Projection::Resource,
                        )
                        .is_ok()
                    && self
                        .authorize_policy_refs(
                            &registry.registry.policy_refs,
                            Some(claim.request.source_uuid),
                            Some(&claim.scope),
                            crate::models::claim::policy::Action::Read,
                            crate::core::policy::authority::Projection::Resource,
                        )
                        .is_ok()
                    && query
                        .scope
                        .as_ref()
                        .is_none_or(|scope| scope == &claim.scope)
                    && (app.settings.source_filter.is_empty()
                        || app
                            .settings
                            .source_filter
                            .contains(&claim.request.source_uuid))
                    && conditions.matches(&claim.context)
                {
                    claims.push(crate::models::claim::ClaimView {
                        claim: claim.clone(),
                        state: self.output(claim.entry_uuid)?.state,
                    });
                }
            }
            if !claims.is_empty() {
                found.push(QueryResult {
                    app: app.uuid,
                    revision: app.revision,
                    claims,
                });
            }
        }
        found.sort_by_key(|result| result.app);
        Ok(found)
    }
}

#[cfg(test)]
mod tests;
