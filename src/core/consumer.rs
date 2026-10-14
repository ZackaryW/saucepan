//! Dedicated CLI consumer lifetimes, shared by source and acquired paths.
use crate::{
    core::{Store, retrieval::RetrievedResource, store::AcquiredResource},
    models::{
        retrieval::{MaterialKind, Ownership},
        source::Resolved,
    },
    utils::{
        lifetime::{inheritable_marker, marker_held},
        path::{beneath, plain_metadata},
    },
};
use anyhow::{Result, ensure};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
};
use uuid::Uuid;

pub(crate) const TEMP_PREFIX: &str = "saucepan-owned-";
const TEMP_MARKER: &str = ".saucepan-consumer";

pub(crate) struct ConsumerMarker {
    file: Option<File>,
    marker: PathBuf,
    temporary: Option<PathBuf>,
}
impl Drop for ConsumerMarker {
    fn drop(&mut self) {
        // Serialize marker inspection/removal with creation and crash sweeping.
        let _coordination = if self.temporary.is_some() {
            match temporary_coordination() {
                Ok(guard) => Some(guard),
                Err(_) => return,
            }
        } else {
            None
        };
        drop(self.file.take());
        if marker_held(&self.marker).is_ok_and(|held| !held) {
            if let Some(root) = &self.temporary {
                let _ = fs::remove_dir_all(root);
            } else {
                let _ = fs::remove_file(&self.marker);
            }
        }
    }
}
pub(crate) fn temporary_marker(
    owner: &mut Option<Rc<tempfile::TempDir>>,
) -> Result<Option<ConsumerMarker>> {
    let Some(owner) = owner.take() else {
        return Ok(None);
    };
    let _coordination = temporary_coordination()?;
    let directory = Rc::try_unwrap(owner)
        .map_err(|_| anyhow::anyhow!("temporary consumer requires exclusive lifetime transfer"))?;
    let marker = directory.path().join(TEMP_MARKER);
    let file = inheritable_marker(&marker)?;
    Ok(Some(ConsumerMarker {
        file: Some(file),
        marker,
        temporary: Some(directory.keep()),
    }))
}

/// Crash leftovers have a named ownership marker. Cleanup requires native proof
/// that every inherited marker handle is closed; age and PID never grant it.
pub(crate) fn sweep_temporary_consumers() -> Result<()> {
    let _coordination = temporary_coordination()?;
    let parent = std::env::temp_dir().canonicalize()?;
    for entry in fs::read_dir(&parent)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX) {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = plain_metadata(&path) else {
            continue;
        };
        if !metadata.is_dir() {
            continue;
        }
        let marker = path.join(TEMP_MARKER);
        let Ok(metadata) = plain_metadata(&marker) else {
            continue;
        };
        if metadata.is_file() && !marker_held(&marker)? {
            // The path was enumerated under this canonical temporary parent.
            ensure!(
                path.parent() == Some(parent.as_path()),
                "temporary cleanup escaped its parent"
            );
            match fs::remove_dir_all(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

fn temporary_coordination() -> Result<crate::utils::lock::LockGuard> {
    Ok(crate::utils::lock::LockGuard::acquire(
        &std::env::temp_dir().join(".saucepan-consumer-coordination.lock"),
        crate::utils::lock::LockMode::Exclusive,
        std::time::Duration::from_secs(30),
    )?)
}

pub(crate) fn has_source_consumers(store: &Store, source: Uuid) -> Result<bool> {
    let prefix = format!("source-live-{source}-");
    let mut live = false;
    for entry in fs::read_dir(store.root().join(".locks"))? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(suffix) = name.strip_prefix(&prefix) else {
            continue;
        };
        if Uuid::parse_str(suffix).is_err() {
            continue;
        }
        let path = beneath(store.root(), &format!(".locks/{name}"))?;
        ensure!(
            plain_metadata(&path)?.is_file(),
            "invalid source consumer marker"
        );
        if marker_held(&path)? {
            live = true;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(live)
}

pub(crate) fn run_retrieved(mut resource: RetrievedResource, mut command: Command) -> Result<i32> {
    sweep_temporary_consumers()?;
    command.env("SAUCEPAN_SOURCE_PATH", resource.source_path());
    command.env(
        "SAUCEPAN_SOURCE_KIND",
        match resource.descriptor.kind {
            MaterialKind::File => "file",
            MaterialKind::Directory => "directory",
            MaterialKind::GitRepository { .. } => "git_repository",
        },
    );
    if let Resolved::Git { commit } = &resource.descriptor.resolved {
        command.env("SAUCEPAN_SOURCE_COMMIT", commit);
    } else {
        command.env_remove("SAUCEPAN_SOURCE_COMMIT");
    }
    let _temporary = temporary_marker(&mut resource._temporary)?;
    let _source = if let Ownership::Managed { .. } = resource.descriptor.ownership {
        let store = resource.store.as_ref().expect("managed retrieval store");
        let crate::models::retrieval::SourceInput::Registered { source_uuid } =
            resource.descriptor.source
        else {
            unreachable!()
        };
        let marker = store.root().join(format!(
            ".locks/source-live-{source_uuid}-{}",
            Uuid::new_v4()
        ));
        Some(ConsumerMarker {
            file: Some(inheritable_marker(&marker)?),
            marker,
            temporary: None,
        })
    } else {
        None
    };
    wait(command)
}

pub(crate) fn run_acquired(mut resource: AcquiredResource, mut command: Command) -> Result<i32> {
    sweep_temporary_consumers()?;
    command.env("SAUCEPAN_RESOURCE_DIR", resource.path());
    let _temporary = temporary_marker(&mut resource._temporary)?;
    if let Some(lease) = &mut resource._lease {
        lease.inherit()?;
    }
    wait(command)
}

fn wait(mut command: Command) -> Result<i32> {
    let status = command.spawn()?.wait()?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
    }
    #[cfg(windows)]
    {
        Ok(status.code().unwrap_or(1))
    }
}

pub(crate) fn temporary_workspace(parent: Option<&Path>) -> std::io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(TEMP_PREFIX);
    match parent {
        Some(parent) => builder.tempdir_in(parent),
        None => builder.tempdir(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{retrieval::Retriever, store::Acquirer},
        models::{
            retrieval::{CachePolicy, RetrievalPlan, SourceInput},
            source::Source,
            store::{AcquisitionPlan, Destination, Selector},
        },
    };
    #[test]
    fn failed_launch_cleans_temporary_source_and_output() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("input");
        fs::write(&input, b"payload").unwrap();
        let request = RetrievalPlan {
            provenance: Vec::new(),
            source: SourceInput::Inline {
                source: Source::Local { path: input },
            },
            selector: Selector::Local,
            asset: Default::default(),
            cache: CachePolicy::Temporary {},
            verification: Default::default(),
            expected_sha256: None,
        };
        let source = Retriever::retrieve_planned_with(&request, &Default::default()).unwrap();
        let source_path = source.source_path().to_owned();
        let acquired = Acquirer::acquire_planned(
            &source,
            &AcquisitionPlan {
                destination: Destination::Temporary {},
                ..Default::default()
            },
        )
        .unwrap();
        let acquired_path = acquired.path().to_owned();
        assert!(run_retrieved(source, Command::new(root.path().join("missing"))).is_err());
        assert!(!source_path.exists());
        assert!(run_acquired(acquired, Command::new(root.path().join("missing"))).is_err());
        assert!(!acquired_path.exists());
    }
}
