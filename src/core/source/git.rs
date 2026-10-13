use crate::{
    models::source::Source,
    utils::{path::plain_metadata, source::git},
};
use anyhow::Result;
use std::fs;

pub(crate) fn normalize(source: Source) -> Result<Source> {
    Ok(match source {
        Source::GithubRepo { repo } => {
            super::remote::repo_path(&repo)?;
            Source::Git {
                origin: format!(
                    "https://github.com/{}.git",
                    repo.trim_end_matches(".git").to_lowercase()
                ),
            }
        }
        Source::Git { origin } => {
            let mut origin = git::normalize(&origin)?;
            if let Some(repo) = origin.strip_prefix("https://github.com/") {
                super::remote::repo_path(repo)?;
                origin = format!(
                    "https://github.com/{}.git",
                    repo.trim_end_matches(".git").to_lowercase()
                );
            }
            Source::Git { origin }
        }
        Source::Local { path } => {
            plain_metadata(&path)?;
            Source::Local {
                path: fs::canonicalize(path)?,
            }
        }
        other => other,
    })
}
