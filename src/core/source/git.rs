use super::{AcquireOptions, Prepared};
use crate::{
    core::Store,
    models::{
        source::{Discovery, Resolved, Source},
        store::{Format, Request, Selector},
    },
    utils::{
        lock::{LockGuard, LockMode},
        path::{beneath, plain_metadata},
        source::git::{self, Git},
        tree::{TreeLimits, copy_tree},
    },
};
use anyhow::{Context, Result, bail, ensure};
use std::{fs, time::Duration};

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

impl Store {
    pub(super) fn prepare_git(
        &self,
        request: &Request,
        options: &AcquireOptions,
        temp: tempfile::TempDir,
    ) -> Result<Prepared> {
        ensure!(
            request.selection.format == Format::Natural,
            "Git requires natural format"
        );
        let (requested, pinned) = match &request.selector {
            Selector::Ref { reference } => (reference.as_str(), false),
            Selector::Tag { tag } => (tag.as_str(), false),
            Selector::Commit { commit } => (commit.as_str(), true),
            _ => bail!("Git requires ref, tag, or commit selector"),
        };
        let _source_lock = LockGuard::acquire(
            &self
                .root
                .join(format!(".locks/source-{}", request.source_uuid)),
            LockMode::Exclusive,
            Duration::from_secs(150),
        )?;
        let mut source = self.source(request.source_uuid)?;
        let Source::Git { origin } = normalize(source.source.clone())? else {
            bail!("expected Git source")
        };
        let work = beneath(&self.root, &format!("source/{}", source.uuid))?;
        fs::create_dir_all(&work)?;
        let repo = work.join("repo");
        let git = Git::default();
        git.repository(&origin, &repo)?;
        let key = serde_json::to_string(&request.selector)?;
        let now = options.now()?;
        let previous = source.discovery.get(&key).cloned();
        let fresh = previous.as_ref().is_some_and(|d| {
            now.checked_sub(d.checked_at)
                .is_some_and(|age| age < source.refresh.ttl_seconds)
        });
        let mut update_checked = false;
        let mut stale = false;
        let resolved = if fresh && !options.force_refresh {
            previous.as_ref().unwrap().resolved.clone()
        } else {
            match git.resolve(&repo, requested, pinned) {
                Ok(commit) => {
                    let resolved = Resolved::Git { commit };
                    source.discovery.insert(
                        key,
                        Discovery {
                            checked_at: now,
                            resolved: resolved.clone(),
                        },
                    );
                    let _lock = self.lock()?;
                    self.save_source(&source)?;
                    update_checked = true;
                    resolved
                }
                Err(error) if options.allow_stale => {
                    stale = true;
                    previous
                        .context(format!("no cached Git result: {error:#}"))?
                        .resolved
                }
                Err(error) => return Err(error),
            }
        };
        let Resolved::Git { commit } = &resolved else {
            bail!("invalid Git cache")
        };
        let complete = temp.path().join("export");
        let export = git.export(&origin, &repo, commit, &work, &complete)?;
        let selected = match &request.selection.folder {
            Some(folder) => beneath(&complete, folder)?,
            None => complete,
        };
        copy_tree(
            &selected,
            &temp.path().join("content"),
            TreeLimits::default(),
        )?;
        Ok(Prepared {
            temp,
            resolved,
            dependencies: export.dependencies,
            update_checked,
            stale,
        })
    }
}
