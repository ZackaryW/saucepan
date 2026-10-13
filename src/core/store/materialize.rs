//! Copy/export/extract only. Provider network access belongs to retrieval.
use crate::{
    core::{retrieval::RetrievedResource, source::git::normalize},
    error::{Error, ErrorCode},
    models::{
        retrieval::MaterialKind,
        source::{Resolved, Source},
        store::{AcquisitionPlan, Format},
    },
    utils::{
        archive::extract_zip,
        path::{beneath, plain_metadata, relative_path},
        source::git::{Git, RepositoryMaterial},
        transfer::copy_bounded,
        tree::{TreeLimits, copy_tree_unhashed},
    },
};
use anyhow::{Context, Result, bail, ensure};
use std::{fs, path::Path};

pub(super) fn copy(
    retrieved: &RetrievedResource,
    request: &AcquisitionPlan,
    staging: &Path,
) -> Result<()> {
    let source = retrieved.source_path();
    let destination = staging.join("content");
    match &retrieved.descriptor.kind {
        MaterialKind::GitRepository { .. } => {
            ensure!(
                request.format == Format::Natural,
                "Git acquisition requires natural format"
            );
            let Source::Git { origin } = normalize(retrieved.source.clone())? else {
                bail!("invalid Git source")
            };
            let Resolved::Git { commit } = &retrieved.descriptor.resolved else {
                bail!("invalid Git provenance")
            };
            let exported = staging.join("export");
            let inventory = Git::default().export_offline(
                RepositoryMaterial {
                    origin: &origin,
                    repository: source,
                    commit,
                    dependencies: source
                        .parent()
                        .context("repository working directory missing")?,
                },
                staging,
                &exported,
            )?;
            ensure!(
                inventory.dependencies == retrieved.descriptor.dependencies,
                Error::new(
                    ErrorCode::IntegrityError,
                    "retrieved Git dependencies changed"
                )
            );
            let selected = match &request.folder {
                Some(folder) => beneath(&exported, folder)?,
                None => exported,
            };
            copy_tree_unhashed(&selected, &destination, TreeLimits::default())?;
        }
        MaterialKind::Directory => {
            ensure!(
                request.format == Format::Natural,
                "directory acquisition requires natural format"
            );
            let selected = match &request.folder {
                Some(folder) => beneath(source, folder)?,
                None => source.to_owned(),
            };
            copy_tree_unhashed(&selected, &destination, TreeLimits::default())?;
        }
        MaterialKind::File => {
            if request.format == Format::Zip {
                extract_zip(
                    fs::File::open(source)?,
                    &destination,
                    request.folder.as_deref(),
                    TreeLimits::default(),
                )?;
            } else {
                ensure!(request.folder.is_none(), "raw file has no folder selection");
                let name = match &request.filename {
                    Some(name) => name.clone(),
                    None => original_filename(retrieved)?,
                };
                relative_path(&name)?;
                ensure!(!name.contains('/'), "filename must be one component");
                fs::create_dir(&destination)?;
                let path = destination.join(name);
                copy_bounded(
                    fs::File::open(source)?,
                    fs::File::create(&path)?,
                    TreeLimits::default().bytes,
                    false,
                )?;
                fs::set_permissions(path, plain_metadata(source)?.permissions())?;
            }
        }
    }
    for binary in &request.bin {
        let path = beneath(&destination, binary)?;
        ensure!(
            plain_metadata(&path)?.is_file(),
            "binary is not a regular file"
        );
        #[cfg(unix)]
        if request.format == Format::Raw
            && matches!(
                retrieved.source,
                Source::Url { .. } | Source::GithubRelease { .. }
            )
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

fn original_filename(retrieved: &RetrievedResource) -> Result<String> {
    match (&retrieved.source, &retrieved.descriptor.resolved) {
        (Source::Local { path }, _) => Ok(path
            .file_name()
            .and_then(|name| name.to_str())
            .context("invalid source filename")?
            .into()),
        (_, Resolved::Release { assets, .. }) => Ok(assets
            .first()
            .context("missing selected asset")?
            .name
            .clone()),
        _ => bail!(Error::new(
            ErrorCode::InvalidRequest,
            "raw download acquisition requires filename"
        )),
    }
}
