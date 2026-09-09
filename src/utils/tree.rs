use super::{fs::staged_directory, path::relative_path};
use std::{fs, io, path::Path};

/// A slash-separated path relative to the walked root. Directories include empty ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub directory: bool,
}

/// List a tree in logical path order. A false predicate prunes that entry/subtree.
/// Reject links, special files, and paths that cannot be represented losslessly.
/// The input tree must remain stable for the duration of walking/reading it.
pub fn entries(
    root: impl AsRef<Path>,
    mut include: impl FnMut(&str, bool) -> bool,
) -> io::Result<Vec<Entry>> {
    fn visit(
        root: &Path,
        prefix: &str,
        include: &mut impl FnMut(&str, bool) -> bool,
        result: &mut Vec<Entry>,
    ) -> io::Result<()> {
        for child in fs::read_dir(root)? {
            let child = child?;
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 filename"))?;
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let directory = child.file_type()?.is_dir();
            if !include(&path, directory) {
                continue;
            }
            relative_path(&path)?;
            metadata(&child.path())?;
            result.push(Entry {
                path: path.clone(),
                directory,
            });
            if directory {
                visit(&child.path(), &path, include, result)?;
            }
        }
        Ok(())
    }
    let root = root.as_ref();
    if !metadata(root)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a directory",
        ));
    }
    let mut result = Vec::new();
    visit(root, "", &mut include, &mut result)?;
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

/// Copy selected entries to a new directory using independent files and staged publication.
/// No destination merging or overwriting; the source must not change during the copy.
pub fn copy_tree(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    include: impl FnMut(&str, bool) -> bool,
) -> io::Result<()> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if parent.canonicalize()?.starts_with(source.canonicalize()?) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination is inside source",
        ));
    }
    let entries = entries(source, include)?;
    staged_directory(destination, |stage| {
        for entry in entries {
            let path = relative_path(&entry.path)?;
            let input = source.join(&path);
            let output = stage.join(&path);
            let meta = metadata(&input)?;
            if entry.directory {
                fs::create_dir(&output)?;
            } else {
                let mut target = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output)?;
                io::copy(&mut fs::File::open(input)?, &mut target)?;
                target.sync_all()?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(
                        &output,
                        fs::Permissions::from_mode(meta.permissions().mode() & 0o777),
                    )?;
                }
                #[cfg(not(unix))]
                let _ = meta;
            }
        }
        Ok(())
    })
}

/// Reject links/reparse points and special files before reading filesystem content.
pub(crate) fn metadata(path: &Path) -> io::Result<fs::Metadata> {
    let meta = fs::symlink_metadata(path)?;
    let linked = meta.file_type().is_symlink();
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        linked || meta.file_attributes() & 0x400 != 0
    };
    if linked || (!meta.is_dir() && !meta.is_file()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "links and special files are unsupported",
        ));
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_in_path_order_and_prunes_with_a_caller_supplied_filter() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("z/empty")).unwrap();
        fs::create_dir_all(dir.path().join("ignored/nested")).unwrap();
        fs::write(dir.path().join("b"), b"b").unwrap();
        fs::write(dir.path().join("a"), b"a").unwrap();
        let result = entries(dir.path(), |path, _| path != "ignored").unwrap();
        assert_eq!(
            result
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "z", "z/empty"]
        );
        assert!(result[3].directory);
        assert!(entries(dir.path().join("absent"), |_, _| true).is_err());
        assert!(entries(dir.path().join("a"), |_, _| true).is_err());
    }

    #[test]
    fn copies_empty_directories_and_files_independently_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let copy = dir.path().join("copy");
        fs::create_dir_all(source.join("empty")).unwrap();
        fs::write(source.join("file"), b"original").unwrap();
        copy_tree(&source, &copy, |_, _| true).unwrap();
        assert!(copy.join("empty").is_dir());
        fs::write(copy.join("file"), b"edited").unwrap();
        assert_eq!(fs::read(source.join("file")).unwrap(), b"original");
        assert!(copy_tree(&source, &copy, |_, _| true).is_err());
        assert_eq!(fs::read(copy.join("file")).unwrap(), b"edited");
    }

    #[test]
    fn destination_inside_source_is_rejected_before_staging() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("copy");
        assert!(copy_tree(dir.path(), &output, |_, _| true).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_including_a_linked_root_without_partial_copy() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("a"), b"a").unwrap();
        std::os::unix::fs::symlink("a", source.join("link")).unwrap();
        assert!(copy_tree(&source, dir.path().join("copy"), |_, _| true).is_err());
        assert!(!dir.path().join("copy").exists());
        std::os::unix::fs::symlink(&source, dir.path().join("root-link")).unwrap();
        assert!(entries(dir.path().join("root-link"), |_, _| true).is_err());
    }
}
