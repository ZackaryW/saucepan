use crate::{
    core::Store,
    models::{
        source::{Resolved, Source},
        store::{Format, Request, Selector},
    },
    utils::{
        path::{beneath, plain_metadata, relative_path},
        tree::{TreeLimits, copy_tree},
    },
};
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, fs};
pub(crate) mod git;
mod remote;

#[derive(Debug, Clone, Default)]
pub struct AcquireOptions {
    pub force_refresh: bool,
    pub allow_stale: bool,
    pub test_now: Option<u64>,
    pub test_github_api: Option<String>,
}
impl AcquireOptions {
    pub(crate) fn now(&self) -> Result<u64> {
        Ok(self.test_now.unwrap_or(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        ))
    }
}

pub(crate) struct Prepared {
    pub temp: tempfile::TempDir,
    pub resolved: Resolved,
    pub dependencies: BTreeMap<String, String>,
    pub update_checked: bool,
    pub stale: bool,
}
impl Store {
    pub(crate) fn prepare(&self, request: &Request, options: &AcquireOptions) -> Result<Prepared> {
        let record = self.source(request.source_uuid)?;
        let temp = tempfile::tempdir_in(self.root.join(".staging"))?;
        match record.source {
            Source::Local { path } => {
                ensure!(
                    matches!(request.selector, Selector::Local),
                    "local source requires local selector"
                );
                ensure!(
                    request.selection.format == Format::Natural,
                    "local source requires natural format"
                );
                let meta = plain_metadata(&path)?;
                if meta.is_dir() {
                    let input = if let Some(folder) = &request.selection.folder {
                        beneath(&path, folder)?
                    } else {
                        path
                    };
                    copy_tree(&input, &temp.path().join("content"), TreeLimits::default())?;
                } else {
                    ensure!(
                        request.selection.folder.is_none(),
                        "file input has no folder selection"
                    );
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| anyhow::anyhow!("invalid filename"))?;
                    relative_path(name)?;
                    ensure!(meta.len() <= TreeLimits::default().bytes, "input too large");
                    fs::create_dir(temp.path().join("content"))?;
                    use std::io::Read;
                    let destination = temp.path().join("content").join(name);
                    let mut input = fs::File::open(&path)?.take(TreeLimits::default().bytes + 1);
                    let mut output = fs::File::create(&destination)?;
                    let size = std::io::copy(&mut input, &mut output)?;
                    let after = plain_metadata(&path)?;
                    ensure!(
                        size == meta.len()
                            && size == after.len()
                            && meta.modified()? == after.modified()?,
                        "local input changed during copy"
                    );
                    fs::set_permissions(destination, meta.permissions())?;
                }
                Ok(Prepared {
                    temp,
                    resolved: Resolved::Local,
                    dependencies: BTreeMap::new(),
                    update_checked: false,
                    stale: false,
                })
            }
            Source::GithubRelease { .. } | Source::Url { .. } => {
                self.prepare_remote(request, options, temp)
            }
            Source::Git { .. } | Source::GithubRepo { .. } => {
                self.prepare_git(request, options, temp)
            }
        }
    }
}
