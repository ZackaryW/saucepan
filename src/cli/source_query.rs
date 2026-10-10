//! CLI input conversion only; matching and permissions belong to core::query.
use crate::core::query::{SourceQuery, SourceType};
use std::path::PathBuf;

#[derive(clap::Args)]
pub(super) struct SourceQueryArgs {
    /// Optional saved JSON query; cannot be combined with filter flags.
    #[arg(conflicts_with_all = ["uuid", "kind", "repo", "origin", "url"])]
    file: Option<PathBuf>,
    /// Exact registered source UUID.
    #[arg(long)]
    uuid: Option<uuid::Uuid>,
    /// Stored provider type (github_repo registrations are stored as git).
    #[arg(long = "type", value_enum)]
    kind: Option<Kind>,
    /// Exact GitHub release repository, such as owner/project.
    #[arg(long)]
    repo: Option<String>,
    /// Exact stored Git origin.
    #[arg(long)]
    origin: Option<String>,
    /// Exact stored URL source.
    #[arg(long)]
    url: Option<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
#[value(rename_all = "snake_case")]
enum Kind {
    Local,
    Url,
    Git,
    GithubRelease,
}

impl SourceQueryArgs {
    pub(super) fn into_query(self) -> anyhow::Result<SourceQuery> {
        if let Some(file) = self.file {
            return Ok(serde_json::from_slice(&std::fs::read(file)?)?);
        }
        Ok(SourceQuery {
            uuid: self.uuid,
            kind: self.kind.map(|kind| match kind {
                Kind::Local => SourceType::Local,
                Kind::Url => SourceType::Url,
                Kind::Git => SourceType::Git,
                Kind::GithubRelease => SourceType::GithubRelease,
            }),
            repo: self.repo,
            origin: self.origin,
            url: self.url,
        })
    }
}
