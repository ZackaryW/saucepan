use super::{SourceQuery, SourceQueryResult, SourceType, SourceView};
use crate::{
    core::{
        Store,
        claim::{policy::authorize_source_discovery, security::Kind},
    },
    error::{Error, ErrorCode},
    models::source::{Source, SourceRecord},
};
use anyhow::{Result, ensure};

impl SourceQuery {
    fn validate(&self) -> Result<()> {
        for value in [&self.repo, &self.origin, &self.url].into_iter().flatten() {
            ensure!(
                !value.is_empty() && value.len() <= 4096,
                Error::new(
                    ErrorCode::InvalidRequest,
                    "source filters must contain 1..4096 UTF-8 bytes"
                )
            );
        }
        Ok(())
    }

    fn matches(&self, record: &SourceRecord) -> bool {
        let (kind, repo, origin, url) = match &record.source {
            Source::Local { .. } => (SourceType::Local, None, None, None),
            Source::Url { url } => (SourceType::Url, None, None, Some(url)),
            Source::Git { origin } => (SourceType::Git, None, Some(origin), None),
            Source::GithubRelease { repo } => (SourceType::GithubRelease, Some(repo), None, None),
            // Registration normalizes this input alias to Git before persistence.
            Source::GithubRepo { .. } => return false,
        };
        self.uuid.is_none_or(|id| id == record.uuid)
            && self.kind.is_none_or(|expected| expected == kind)
            && self
                .repo
                .as_ref()
                .is_none_or(|expected| Some(expected) == repo)
            && self
                .origin
                .as_ref()
                .is_none_or(|expected| Some(expected) == origin)
            && self
                .url
                .as_ref()
                .is_none_or(|expected| Some(expected) == url)
    }
}

impl Store {
    /// Metadata-only search. Without an app session this is anonymous discovery,
    /// even when called through the trusted administrative Store handle.
    pub fn query_sources(&self, query: &SourceQuery) -> Result<SourceQueryResult> {
        query.validate()?;
        let _lock = self.lock()?;
        let application = self
            .session
            .as_ref()
            .map(|session| -> Result<_> {
                let app = self.application(session.app)?;
                let registry = self.registry(session.app)?;
                self.check_session(&registry)?;
                authorize_source_discovery(&registry.registry.access, app.uuid, None)?;
                self.authorize_policy_refs(
                    &registry.registry.policy_refs,
                    None,
                    None,
                    crate::models::claim::policy::Action::Discover,
                    crate::core::policy::authority::Projection::ActionOnly,
                )?;
                Ok((app, registry))
            })
            .transpose()?;
        let snapshot = self.metadata.snapshot()?;
        let policies = crate::core::policy::persistence::load(&self.metadata, &snapshot)?;
        let mut sources = Vec::new();
        for reference in snapshot
            .catalog
            .records
            .values()
            .filter(|reference| reference.identity.kind == Kind::Source)
        {
            let mut record: SourceRecord =
                snapshot.read(self.metadata.authenticator(), reference.identity)?;
            ensure!(
                Some(record.uuid) == reference.identity.member,
                Error::new(ErrorCode::IntegrityError, "source identity mismatch")
            );
            crate::core::policy::source::hydrate(&mut record, &policies)?;
            let visible = match &application {
                Some((app, registry)) => {
                    authorize_source_discovery(
                        &registry.registry.access,
                        app.uuid,
                        Some(record.uuid),
                    )
                    .is_ok()
                        && (app.settings.source_filter.is_empty()
                            || app.settings.source_filter.contains(&record.uuid))
                }
                None => record.public_discovery,
            };
            let refs = application
                .as_ref()
                .map(|(_, registry)| &registry.registry.policy_refs)
                .cloned()
                .unwrap_or_default();
            if visible
                && self
                    .authorize_policy_refs(
                        &refs,
                        Some(record.uuid),
                        None,
                        crate::models::claim::policy::Action::Discover,
                        crate::core::policy::authority::Projection::SourceDiscovery,
                    )
                    .is_ok()
                && query.matches(&record)
            {
                sources.push(SourceView {
                    uuid: record.uuid,
                    source: record.source,
                    public_discovery: record.public_discovery,
                });
            }
        }
        sources.sort_by_key(|source| source.uuid);
        Ok(SourceQueryResult {
            generation: snapshot.catalog.generation,
            sources,
        })
    }
}

#[cfg(test)]
mod tests;
