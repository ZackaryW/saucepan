use std::{fs::File, io, path::Path};

use tempfile::NamedTempFile;

/// Create relative directories under a trusted root without merging different
/// spellings that alias on this filesystem. Existing components must be directories.
pub fn create_directories(root: impl AsRef<Path>, relative: impl AsRef<Path>) -> io::Result<()> {
    let mut parent = root.as_ref().to_owned();
    for component in relative.as_ref().components() {
        let std::path::Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected relative directory components",
            ));
        };
        let next = parent.join(name);
        match std::fs::create_dir(&next) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if !super::tree::metadata(&next)?.is_dir()
                    || !std::fs::read_dir(&parent)?
                        .any(|entry| entry.is_ok_and(|entry| entry.file_name() == name))
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "colliding directory paths",
                    ));
                }
            }
            Err(error) => return Err(error),
        }
        parent = next;
    }
    Ok(())
}

/// Prepare a new directory privately, then publish it at an absent destination.
/// The parent must exist. Callers must serialize competing publishers and trust
/// the parent path; this helper is not a filesystem sandbox or a transaction log.
pub fn staged_directory(
    path: impl AsRef<Path>,
    prepare: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let path = path.as_ref();
    let absent = || match path.symlink_metadata() {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination exists",
        )),
    };
    absent()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = tempfile::tempdir_in(parent)?;
    prepare(stage.path())?;
    absent()?;
    std::fs::rename(stage.path(), path)?;
    Ok(())
}

/// Write a sibling temporary file, sync it, then atomically replace the destination.
///
/// The parent must exist. The callback can stream any format into the file.
/// This does not provide multi-file transactions, writer locking, preservation
/// of destination metadata, or directory-entry durability after power loss.
pub fn atomic_write(
    path: impl AsRef<Path>,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = NamedTempFile::new_in(parent)?;
    write(staged.as_file_mut())?;
    staged.as_file().sync_all()?;
    staged
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};
    use tempfile::tempdir;

    #[test]
    fn directory_publication_is_complete_and_preserves_occupied_destinations() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("output");
        staged_directory(&output, |stage| {
            fs::create_dir(stage.join("nested"))?;
            fs::write(stage.join("nested/file"), b"complete")?;
            assert!(!output.exists());
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(output.join("nested/file")).unwrap(), b"complete");
        assert!(staged_directory(&output, |_| panic!("occupied")).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_directory_preparation_leaves_no_partial_result() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("output");
        assert!(
            staged_directory(&output, |stage| {
                fs::write(stage.join("partial"), b"partial")?;
                Err(io::Error::other("failed"))
            })
            .is_err()
        );
        assert!(!output.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn creates_a_file_only_after_the_writer_finishes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("result");
        atomic_write(&path, |file| {
            file.write_all(b"new")?;
            assert!(!path.exists());
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn keeps_old_bytes_visible_until_replacement() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("result");
        fs::write(&path, b"old").unwrap();
        atomic_write(&path, |file| {
            file.write_all(b"replacement")?;
            assert_eq!(fs::read(&path)?, b"old");
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
    }

    #[test]
    fn failed_writer_preserves_old_file_and_cleans_staging() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("result");
        fs::write(&path, b"old").unwrap();
        let error = atomic_write(&path, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("producer failed"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "producer failed");
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_publication_preserves_destination_and_cleans_staging() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("occupied");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"kept").unwrap();
        assert!(atomic_write(&path, |file| file.write_all(b"new")).is_err());
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"kept");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn supports_empty_content_and_reports_missing_parents() {
        let dir = tempdir().unwrap();
        let empty = dir.path().join("empty");
        atomic_write(&empty, |_| Ok(())).unwrap();
        assert!(fs::read(empty).unwrap().is_empty());
        let missing = dir.path().join("missing").join("file");
        assert!(atomic_write(missing, |_| panic!("writer must not run")).is_err());
        assert!(!dir.path().join("missing").exists());
    }
}
