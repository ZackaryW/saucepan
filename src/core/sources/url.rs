use super::{Prepared, RemoteUnavailable};
use crate::core::{content, models::Download};
use crate::utils::archive;
use anyhow::{Result, ensure};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::Path,
    time::Duration,
};

pub(crate) fn prepare(url: &str, download: &Download, work: &Path) -> Result<Prepared> {
    let temp = tempfile::tempdir_in(work)?;
    let downloaded = temp.path().join("download");
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent.get(url).call().map_err(|_| RemoteUnavailable)?;
    let mut target = fs::File::create(&downloaded)?;
    let count = io::copy(
        &mut response.body_mut().as_reader().take(super::MAX_BYTES + 1),
        &mut target,
    )
    .map_err(|_| RemoteUnavailable)?;
    ensure!(count <= super::MAX_BYTES, "download exceeds supported size");
    target.sync_all()?;
    drop(target);
    let tree = temp.path().join("tree");
    let mut executables = std::collections::BTreeSet::new();
    match download {
        Download::File { name } => {
            fs::create_dir(&tree)?;
            fs::rename(
                &downloaded,
                tree.join(crate::utils::path::native_relative_path(name)?),
            )?;
        }
        Download::Zip => {
            archive::extract_zip(fs::File::open(&downloaded)?, &tree, None, super::ZIP_LIMITS)?;
            let mut archive = zip::ZipArchive::new(fs::File::open(&downloaded)?)?;
            for index in 0..archive.len() {
                let file = archive.by_index(index)?;
                if !file.is_dir() && file.unix_mode().is_some_and(|mode| mode & 0o111 != 0) {
                    executables.insert(file.name().to_owned());
                }
            }
        }
    }
    let mut files = content::files(&tree)?;
    for (path, record) in &mut files {
        record.executable = executables.contains(path);
    }
    let revision = content::digest(&files)?;
    Ok(Prepared {
        directory: temp,
        revision,
        dependencies: BTreeMap::new(),
        executables: Some(executables),
    })
}
