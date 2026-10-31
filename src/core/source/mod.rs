use crate::{
    core::Store,
    models::{
        source::Resolved,
        store::{OutputPlan, Selector},
    },
};
use anyhow::Result;
use std::collections::BTreeMap;
mod composition;
pub(crate) mod credentials;
pub(crate) mod git;
pub(crate) mod remote;
pub(crate) use remote::check_loopback;
pub(crate) mod retrieve;

#[derive(Debug, Clone, Default)]
pub struct AcquireOptions {
    pub test_now: Option<u64>,
    pub test_github_api: Option<String>,
    /// Fake `gh` executable; without it, fixture endpoints never call `gh`.
    pub test_gh: Option<std::path::PathBuf>,
    pub test_github_raw: Option<String>,
    /// Replaces `https://{host}` for `git_raw`; loopback only.
    pub test_git_raw: Option<String>,
}
impl AcquireOptions {
    pub(crate) fn uses_test_endpoints(&self) -> bool {
        self.test_github_api.is_some()
            || self.test_github_raw.is_some()
            || self.test_git_raw.is_some()
    }
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
    /// Recreate saved provenance without consulting a moving selector. The caller
    /// compares the complete manifest and dependencies before publication.
    pub(crate) fn prepare_exact(
        &self,
        entry: &crate::models::store::OutputEntry,
        options: &AcquireOptions,
    ) -> Result<Prepared> {
        match &entry.resolved {
            Resolved::Local => self.prepare(&entry.request, options),
            Resolved::Git { commit } => {
                let mut pinned = entry.request.clone();
                pinned.selector = Selector::Commit {
                    commit: commit.clone(),
                };
                self.prepare(&pinned, options)
            }
            Resolved::Release { .. } | Resolved::Url { .. } => {
                self.prepare_exact_download(&entry.request, &entry.resolved, options)
            }
        }
    }
    pub(crate) fn prepare(
        &self,
        request: &OutputPlan,
        options: &AcquireOptions,
    ) -> Result<Prepared> {
        self.prepare_composed(request, options)
    }
}
