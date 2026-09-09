use super::{content, models::Source};
use crate::utils::{path::native_relative_path, tree};
use anyhow::{Context, Result};
use std::{collections::BTreeMap, fs, path::Path};
pub(crate) mod git;
mod url;

pub(crate) const MAX_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub(crate) const ZIP_LIMITS: crate::utils::archive::Limits = crate::utils::archive::Limits {
    entries: 1_000_000,
    bytes: MAX_BYTES,
};

#[derive(Debug)]
pub(crate) struct RemoteUnavailable;
impl std::fmt::Display for RemoteUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("remote update check failed")
    }
}
impl std::error::Error for RemoteUnavailable {}

pub(crate) struct Prepared {
    pub directory: tempfile::TempDir,
    pub revision: String,
    pub dependencies: BTreeMap<String, String>,
    pub executables: Option<std::collections::BTreeSet<String>>,
}

pub(crate) fn prepare(source: &Source, work: &Path, commit: Option<&str>) -> Result<Prepared> {
    match source {
        Source::Url {
            url: locator,
            download,
        } => url::prepare(locator, download, work),
        Source::Local { path } => {
            let temp = tempfile::tempdir_in(work)?;
            let output = temp.path().join("tree");
            if tree::metadata(path)?.is_dir() {
                tree::copy_tree(path, &output, |_, _| true)?;
            } else {
                fs::create_dir(&output)?;
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("invalid local filename")?;
                fs::copy(path, output.join(native_relative_path(name)?))?;
            }
            let revision = content::digest(&content::files(&output)?)?;
            Ok(Prepared {
                directory: temp,
                revision,
                dependencies: BTreeMap::new(),
                executables: None,
            })
        }
        Source::Git { .. } => git::prepare(source, work, commit),
    }
}
