//! ZIP stream helpers; retention, source selection policy, and provenance belong to callers.
use super::{
    fs::{create_directories as directories, staged_directory},
    path::native_relative_path,
    tree,
};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Caller-selected extraction bounds, including entries outside a selected folder.
#[derive(Clone, Copy)]
pub struct Limits {
    pub entries: usize,
    pub bytes: u64,
}

/// Write a stable, sorted ZIP of a stable input tree, including empty directories.
/// The predicate controls exclusions. The writer must be outside the source tree.
/// Stream errors may leave partial output; use `atomic_write` when publishing a file.
pub fn write_zip<W: Write + Seek>(
    root: impl AsRef<Path>,
    writer: W,
    include: impl FnMut(&str, bool) -> bool,
) -> io::Result<W> {
    write_zip_with(root, writer, include, |_, options| options)
}

/// Export with caller-selected per-entry options, for example logical executable
/// metadata carried by an upstream format rather than the host filesystem.
pub fn write_zip_with<W: Write + Seek>(
    root: impl AsRef<Path>,
    writer: W,
    include: impl FnMut(&str, bool) -> bool,
    mut options_for: impl FnMut(&tree::Entry, SimpleFileOptions) -> SimpleFileOptions,
) -> io::Result<W> {
    let root = root.as_ref();
    let mut zip = ZipWriter::new(writer);
    for entry in tree::entries(root, include)? {
        let input = root.join(native_relative_path(&entry.path)?);
        let metadata = tree::metadata(&input)?;
        let options = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
        #[cfg(unix)]
        let options = {
            use std::os::unix::fs::PermissionsExt;
            options.unix_permissions(metadata.permissions().mode() & 0o777)
        };
        #[cfg(not(unix))]
        let _ = metadata;
        let options = options_for(&entry, options);
        if entry.directory {
            zip.add_directory(entry.path, options)?;
        } else {
            zip.start_file(entry.path, options)?;
            io::copy(&mut fs::File::open(input)?, &mut zip)?;
        }
    }
    Ok(zip.finish()?)
}

/// Extract a ZIP, or one directory within it, into a new directory. Selected folder
/// contents start at the output root. Stored and Deflate compression are supported.
/// Reject invalid paths, links, collisions, malformed data, and exceeded bounds.
/// All writes are staged; an error never publishes a partial destination.
pub fn extract_zip(
    reader: impl Read + Seek,
    destination: impl AsRef<Path>,
    folder: Option<&str>,
    limits: Limits,
) -> io::Result<()> {
    if let Some(folder) = folder {
        native_relative_path(folder)?;
    }
    let prefix = folder.map(|folder| format!("{folder}/"));
    let mut zip = checked_zip(reader)?;
    if zip.len() > limits.entries {
        return Err(invalid("ZIP entry limit exceeded"));
    }
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    let mut selected = Vec::new();
    let mut found = folder.is_none();
    for index in 0..zip.len() {
        let file = zip.by_index(index)?;
        let name =
            std::str::from_utf8(file.name_raw()).map_err(|_| invalid("non-UTF-8 ZIP path"))?;
        let name = if file.is_dir() {
            name.strip_suffix('/').unwrap_or(name)
        } else {
            name
        };
        native_relative_path(name)?;
        if !names.insert(name.to_owned()) {
            return Err(invalid("duplicate ZIP path"));
        }
        let kind = file.unix_mode().unwrap_or(0) & 0o170000;
        if file.is_symlink()
            || !matches!(kind, 0 | 0o100000 | 0o040000)
            || (kind == 0o040000 && !file.is_dir())
            || (kind == 0o100000 && file.is_dir())
        {
            return Err(invalid("ZIP links and special files are unsupported"));
        }
        total = total
            .checked_add(file.size())
            .filter(|&n| n <= limits.bytes)
            .ok_or_else(|| invalid("ZIP byte limit exceeded"))?;
        let relative = match &prefix {
            Some(prefix) if name == folder.unwrap_or_default() && file.is_dir() => {
                found = true;
                continue;
            }
            Some(prefix) => match name.strip_prefix(prefix) {
                Some(path) => path,
                None => continue,
            },
            None => name,
        };
        found = true;
        selected.push((index, native_relative_path(relative)?));
    }
    if !found {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "ZIP directory not found",
        ));
    }
    staged_directory(destination, |stage| {
        for (index, relative) in selected {
            let mut file = zip.by_index(index)?;
            let output = stage.join(&relative);
            if file.is_dir() {
                directories(stage, &relative)?;
            } else {
                directories(stage, relative.parent().unwrap_or(Path::new("")))?;
                let mut writer = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output)?;
                let expected = file.size();
                let copied = io::copy(
                    &mut (&mut file).take(expected.saturating_add(1)),
                    &mut writer,
                )?;
                if copied != expected {
                    return Err(invalid("ZIP entry size mismatch"));
                }
                writer.sync_all()?;
                #[cfg(unix)]
                if let Some(mode) = file.unix_mode() {
                    use std::os::unix::fs::PermissionsExt;
                    // Preserve executable bits, never restore special permission bits.
                    fs::set_permissions(
                        &output,
                        fs::Permissions::from_mode(0o644 | (mode & 0o111)),
                    )?;
                }
            }
        }
        Ok(())
    })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// ZipArchive indexes names and silently replaces duplicate records. Require its
// surviving headers to cover the central directory without omitted entries.
// ZIP64 uses the same fixed central header and variable-field length positions.
fn checked_zip<R: Read + Seek>(reader: R) -> io::Result<ZipArchive<R>> {
    let mut zip = ZipArchive::new(reader)?;
    let start = zip.central_directory_start();
    let mut offsets = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        offsets.push(zip.by_index_raw(index)?.central_header_start());
    }
    offsets.sort_unstable();
    let mut reader = zip.into_inner();
    reader.seek(SeekFrom::Start(start))?;
    for offset in offsets {
        if reader.stream_position()? != offset {
            return Err(invalid("duplicate or omitted ZIP entry"));
        }
        let mut header = [0; 46];
        reader.read_exact(&mut header)?;
        if &header[..4] != b"PK\x01\x02" {
            return Err(invalid("invalid ZIP central header"));
        }
        let length: u64 = [28, 30, 32]
            .into_iter()
            .map(|i| u64::from(u16::from_le_bytes([header[i], header[i + 1]])))
            .sum();
        reader.seek(SeekFrom::Current(length as i64))?;
    }
    Ok(ZipArchive::new(reader)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Cursor};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
    const LIMITS: Limits = Limits {
        entries: 100,
        bytes: 100_000,
    };

    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in files {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn duplicate_raw_zip_names_are_rejected_instead_of_silently_selecting_one() {
        let mut bytes = zip(&[("a", b"first"), ("b", b"second")]);
        for offset in 0..bytes.len() - 46 {
            let name_offset = if bytes[offset..].starts_with(b"PK\x01\x02") {
                Some(offset + 46)
            } else if bytes[offset..].starts_with(b"PK\x03\x04") {
                Some(offset + 30)
            } else {
                None
            };
            if let Some(name_offset) = name_offset
                && bytes[name_offset] == b'b'
            {
                bytes[name_offset] = b'a';
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output");
        assert!(extract_zip(Cursor::new(bytes), &output, None, LIMITS).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn round_trip_is_sorted_stable_filtered_and_preserves_empty_directories() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir_all(source.join("nested/.git")).unwrap();
        fs::create_dir_all(source.join("empty")).unwrap();
        fs::write(source.join("nested/.git/config"), b"excluded").unwrap();
        fs::write(source.join("nested/é.txt"), b"content").unwrap();
        fs::write(source.join(".gitignore"), b"keep").unwrap();
        let export = || {
            write_zip(&source, Cursor::new(Vec::new()), |path, _| {
                !path.split('/').any(|p| p == ".git")
            })
            .unwrap()
            .into_inner()
        };
        let bytes = export();
        assert_eq!(bytes, export());
        let mut zip = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let names: Vec<_> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect();
        assert_eq!(names, [".gitignore", "empty/", "nested/", "nested/é.txt"]);
        let output = dir.path().join("output");
        extract_zip(Cursor::new(bytes), &output, None, LIMITS).unwrap();
        assert_eq!(fs::read(output.join("nested/é.txt")).unwrap(), b"content");
        assert!(output.join("empty").is_dir());
        assert!(!output.join("nested/.git").exists());
    }

    #[test]
    fn folder_selection_strips_only_the_selected_directory() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = zip(&[("a/file", b"a"), ("ab/file", b"ab"), ("b/file", b"b")]);
        for (folder, expected) in [("a", b"a".as_slice()), ("b", b"b".as_slice())] {
            let output = dir.path().join(folder);
            extract_zip(Cursor::new(&bytes), &output, Some(folder), LIMITS).unwrap();
            assert_eq!(fs::read(output.join("file")).unwrap(), expected);
            assert_eq!(fs::read_dir(output).unwrap().count(), 1);
        }
        for selection in ["missing", "a/file", "../a"] {
            let output = dir.path().join("invalid");
            assert!(extract_zip(Cursor::new(&bytes), &output, Some(selection), LIMITS).is_err());
            assert!(!output.exists());
        }
    }

    #[test]
    fn unsafe_paths_and_file_directory_conflicts_never_publish() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output");
        for name in [
            "../escape",
            "/absolute",
            "C:/drive",
            "a\\escape",
            "a/../escape",
            "a//b",
            "a/file:stream",
        ] {
            let bytes = zip(&[("good", b"good"), (name, b"bad")]);
            assert!(
                extract_zip(Cursor::new(bytes), &output, None, LIMITS).is_err(),
                "{name}"
            );
            assert!(!output.exists());
        }
        let bytes = zip(&[("a", b"file"), ("a/b", b"conflict")]);
        assert!(extract_zip(Cursor::new(bytes), &output, None, LIMITS).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn links_and_corrupt_file_bytes_fail_without_partial_publication() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink("link", "../outside", SimpleFileOptions::default())
            .unwrap();
        assert!(extract_zip(writer.finish().unwrap(), &output, None, LIMITS).is_err());
        let mut bytes = zip(&[("file", b"original")]);
        let start = ZipArchive::new(Cursor::new(&bytes))
            .unwrap()
            .by_index(0)
            .unwrap()
            .data_start()
            .unwrap() as usize;
        bytes[start] ^= 1;
        assert!(extract_zip(Cursor::new(bytes), &output, None, LIMITS).is_err());
        assert!(extract_zip(Cursor::new(b"not a zip"), &output, None, LIMITS).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn bounds_and_occupied_destinations_preserve_existing_data() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output");
        let bytes = zip(&[("a", b"1234"), ("b", b"5678")]);
        for limits in [
            Limits {
                entries: 1,
                bytes: 100,
            },
            Limits {
                entries: 100,
                bytes: 7,
            },
        ] {
            assert!(extract_zip(Cursor::new(&bytes), &output, None, limits).is_err());
            assert!(!output.exists());
        }
        extract_zip(
            Cursor::new(&bytes),
            &output,
            None,
            Limits {
                entries: 2,
                bytes: 8,
            },
        )
        .unwrap();
        fs::write(output.join("a"), b"edited").unwrap();
        assert!(extract_zip(Cursor::new(&bytes), &output, None, LIMITS).is_err());
        assert_eq!(fs::read(output.join("a")).unwrap(), b"edited");
    }

    #[cfg(windows)]
    #[test]
    fn windows_collisions_and_unrepresentable_names_fail() {
        let dir = tempfile::tempdir().unwrap();
        for files in [
            vec![("A/file", b"a".as_slice()), ("a/other", b"b".as_slice())],
            vec![("file.", b"a")],
            vec![("NUL.txt", b"a")],
        ] {
            let output = dir.path().join("output");
            assert!(extract_zip(Cursor::new(zip(&files)), &output, None, LIMITS).is_err());
            assert!(!output.exists());
        }
    }
}
