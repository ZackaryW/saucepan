//! Application-scoped claim discovery. Results carry no retained resource lifetime.
mod matching;
pub mod model;
use crate::core::Store;
use anyhow::Result;
pub use model::{ClaimQuery, Condition, Operator, QueryResult};

impl Store {
    /// Read one coherent filtered application view, in claim UUID order.
    /// Call `lease` on a returned output, or `lease_claim` for the current claim
    /// selection, before consuming resources that must survive reclaim.
    pub fn query_claims(&self, query: &ClaimQuery) -> Result<QueryResult> {
        let conditions = matching::CompiledConditions::compile(&query.conditions)?;
        let _lock = self.lock()?;
        let application = self.application(query.app)?;
        let revision = application.revision;
        let claims =
            self.claim_views_locked(application, |claim| conditions.matches(&claim.context))?;
        Ok(QueryResult {
            app: query.app,
            revision,
            claims,
        })
    }
}

#[cfg(test)]
mod tests;
