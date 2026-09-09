use super::models::FileRecord;
use crate::utils::{hash::sha256, json::sorted_json, path::native_relative_path, tree};
use anyhow::{Result, ensure};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn digest(value: &impl serde::Serialize) -> Result<String> {
    Ok(sha256(sorted_json(value)?.as_slice())?)
}

pub(crate) fn files(root: &Path) -> Result<BTreeMap<String, FileRecord>> {
    tree::entries(root, |_, _| true)?
        .into_iter()
        .map(|entry| {
            let path = root.join(native_relative_path(&entry.path)?);
            #[cfg(not(unix))]
            let executable = false;
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                !entry.directory && tree::metadata(&path)?.permissions().mode() & 0o111 != 0
            };
            let digest = if entry.directory {
                None
            } else {
                Some(sha256(fs::File::open(path)?)?)
            };
            Ok((entry.path, FileRecord { digest, executable }))
        })
        .collect()
}

pub(crate) fn folder(root: &Path, selection: Option<&str>) -> Result<PathBuf> {
    let path = match selection {
        Some(path) => root.join(native_relative_path(path)?),
        None => root.to_owned(),
    };
    // Check every component, including symlinks in an ancestor of the selection.
    let mut checked = root.to_owned();
    ensure!(
        tree::metadata(&checked)?.is_dir(),
        "content root is not a directory"
    );
    if let Some(selection) = selection {
        for component in native_relative_path(selection)?.components() {
            checked.push(component);
            ensure!(
                tree::metadata(&checked)?.is_dir(),
                "selection is not a directory"
            );
        }
    }
    Ok(path)
}

pub(crate) fn verify(
    root: &Path,
    expected: &BTreeMap<String, FileRecord>,
    checked: bool,
) -> Result<()> {
    if checked {
        let actual = files(root)?;
        #[cfg(windows)]
        let actual = actual
            .into_iter()
            .map(|(path, mut record)| {
                // Windows has no Unix executable bit; its logical value remains authenticated metadata.
                if let Some(expected) = expected.get(&path) {
                    record.executable = expected.executable;
                }
                (path, record)
            })
            .collect::<BTreeMap<_, _>>();
        ensure!(&actual == expected, "content digest mismatch");
    } else {
        let actual = tree::entries(root, |_, _| true)?;
        ensure!(
            actual.len() == expected.len()
                && actual.iter().all(|entry| expected
                    .get(&entry.path)
                    .is_some_and(|record| record.digest.is_none() == entry.directory)),
            "cached content tree is incomplete or has unexpected entries"
        );
    }
    Ok(())
}
