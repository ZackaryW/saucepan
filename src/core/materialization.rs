//! Independent trees: validate before writes, links last, no recursive deletion.
use super::{models::*, snapshots};
use crate::utils;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
};

pub(super) struct Prepared {
    pub staging: tempfile::TempDir,
    pub entries: BTreeMap<String, MaterializedEntry>,
}

pub(super) fn prepare(
    parent: &Path,
    bytes: &[u8],
    expected_tree: &str,
    verify: bool,
) -> Result<Prepared> {
    let entries = snapshots::decode(bytes, None)?;
    if verify && snapshots::tree_digest(&entries) != expected_tree {
        return Err(invalid(
            "archive tree does not match authenticated artifact",
        ));
    }
    let mut baseline = BTreeMap::new();
    for (name, (mode, bytes)) in &entries {
        validate_name(name)?;
        if mode & 0o7000 != 0 {
            return Err(invalid("special permission bits are not supported"));
        }
        baseline.insert(
            name.clone(),
            MaterializedEntry {
                mode: *mode,
                size: bytes.len() as u64,
                digest: hash(bytes),
                implicit_directory: false,
            },
        );
    }
    for name in entries.keys() {
        let mut parent = name.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            baseline
                .entry(prefix.into())
                .or_insert_with(|| MaterializedEntry {
                    mode: 0o40755,
                    size: 0,
                    digest: hash(&[]),
                    implicit_directory: true,
                });
            parent = prefix;
        }
    }
    if baseline.len() > 100_000 {
        return Err(invalid("materialization entry limit exceeded"));
    }
    let staging = tempfile::Builder::new()
        .prefix("materialize-")
        .tempdir_in(parent)
        .map_err(|_| invalid("cannot create materialization staging"))?;
    // Every directory is created exclusively. The real filesystem detects
    // case/normalization aliases, including collisions in implicit parents.
    for (name, entry) in &baseline {
        if entry.mode & 0o170000 == 0o040000 {
            fs::create_dir(staging.path().join(name))
                .map_err(|_| invalid("unrepresentable or conflicting materialization directory"))?;
        }
    }
    for (name, (mode, bytes)) in &entries {
        if mode & 0o170000 == 0o100000 {
            let path = staging.path().join(name);
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| invalid("unrepresentable or conflicting materialization file"))?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| invalid("cannot flush materialization file"))?;
            set_mode(&path, *mode)?;
        }
    }
    for (name, (mode, bytes)) in &entries {
        if mode & 0o170000 == 0o120000 {
            let target = std::str::from_utf8(bytes).map_err(|_| invalid("invalid link target"))?;
            let path = staging.path().join(name);
            #[cfg(unix)]
            let result = std::os::unix::fs::symlink(target, &path);
            #[cfg(windows)]
            let result = {
                // Follow only the already validated relative link graph to
                // determine the type required by Windows, never for extraction.
                if link_targets_directory(name, target, &entries)? {
                    std::os::windows::fs::symlink_dir(target, &path)
                } else {
                    std::os::windows::fs::symlink_file(target, &path)
                }
            };
            result.map_err(|_| {
                Error::new(
                    ErrorKind::Compatibility,
                    "platform cannot create the required relative symlink",
                )
            })?;
        }
    }
    for (name, entry) in baseline.iter().rev() {
        if entry.mode & 0o170000 == 0o040000 {
            let path = staging.path().join(name);
            set_mode(&path, entry.mode)?;
            sync_dir(&path)?;
        }
    }
    sync_dir(staging.path())?;
    check(staging.path(), &baseline, false)?;
    Ok(Prepared {
        staging,
        entries: baseline,
    })
}

fn validate_name(name: &str) -> Result<()> {
    super::recipes::validate_selection(name)?;
    if name == "." {
        return Err(invalid("materialized entry cannot name its root"));
    }
    #[cfg(windows)]
    for part in name.split('/') {
        let stem = part.split('.').next().unwrap_or_default().to_uppercase();
        if part.ends_with(['.', ' '])
            || part.contains(['<', '>', '"', '|', '?', '*'])
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(
                    &stem[3..],
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                ))
        {
            return Err(invalid(
                "archive name cannot be represented on this platform",
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn link_targets_directory(
    name: &str,
    target: &str,
    entries: &BTreeMap<String, (u32, Vec<u8>)>,
) -> Result<bool> {
    let mut resolved: Vec<String> = name.split('/').map(str::to_owned).collect();
    resolved.pop();
    let mut pending: std::collections::VecDeque<String> =
        target.split('/').map(str::to_owned).collect();
    let mut hops = 0;
    while let Some(part) = pending.pop_front() {
        match part.as_str() {
            "" | "." => continue,
            ".." => {
                resolved
                    .pop()
                    .ok_or_else(|| invalid("link escapes materialization"))?;
                continue;
            }
            _ => resolved.push(part),
        }
        if let Some((mode, target)) = entries.get(&resolved.join("/"))
            && mode & 0o170000 == 0o120000
        {
            hops += 1;
            if hops > 64 {
                return Err(invalid("link cycle or depth limit"));
            }
            resolved.pop();
            for part in std::str::from_utf8(target)
                .map_err(|_| invalid("invalid link target"))?
                .split('/')
                .rev()
            {
                pending.push_front(part.into());
            }
        }
    }
    let target = resolved.join("/");
    if target.is_empty()
        || entries
            .get(&target)
            .is_some_and(|(mode, _)| mode & 0o170000 == 0o040000)
        || entries
            .keys()
            .any(|name| name.starts_with(&format!("{target}/")))
    {
        return Ok(true);
    }
    if entries
        .get(&target)
        .is_some_and(|(mode, _)| mode & 0o170000 == 0o100000)
    {
        return Ok(false);
    }
    Err(Error::new(
        ErrorKind::Compatibility,
        "Windows cannot determine the required dangling symlink type",
    ))
}

/// Walk without following any links. Partial mode permits only missing entries,
/// allowing interrupted owned cleanup to resume without blessing extra files.
pub(super) fn check(
    root: &Path,
    baseline: &BTreeMap<String, MaterializedEntry>,
    partial: bool,
) -> Result<String> {
    if baseline.len() > 100_000 {
        return Err(invalid("materialization entry limit exceeded"));
    }
    for name in baseline.keys() {
        validate_name(name)?;
    }
    let metadata = match fs::symlink_metadata(root) {
        Err(e) if partial && e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        result => result.map_err(|_| invalid("materialization directory is missing"))?,
    };
    if !metadata.is_dir() || reparse(&metadata) {
        return Err(invalid("materialization root is not an owned directory"));
    }
    let mut pending = vec![String::new()];
    let mut found = BTreeMap::new();
    let mut logical = BTreeMap::new();
    let mut total = 0u64;
    while let Some(parent) = pending.pop() {
        for child in fs::read_dir(root.join(&parent))
            .map_err(|_| invalid("cannot inspect materialization"))?
        {
            let child = child.map_err(|_| invalid("cannot inspect materialization entry"))?;
            let leaf = child
                .file_name()
                .into_string()
                .map_err(|_| invalid("non-UTF-8 materialization entry"))?;
            let name = if parent.is_empty() {
                leaf
            } else {
                format!("{parent}/{leaf}")
            };
            let expected = baseline
                .get(&name)
                .ok_or_else(|| invalid("materialization contains unowned entries"))?;
            let metadata = fs::symlink_metadata(child.path())
                .map_err(|_| invalid("cannot inspect materialized metadata"))?;
            let kind = expected.mode & 0o170000;
            let bytes = if kind == 0o120000 {
                if !metadata.file_type().is_symlink() {
                    return Err(invalid("materialized link type changed"));
                }
                fs::read_link(child.path())
                    .map_err(|_| invalid("cannot read materialized link"))?
                    .to_str()
                    .ok_or_else(|| invalid("non-UTF-8 materialized link"))?
                    .as_bytes()
                    .to_vec()
            } else {
                if reparse(&metadata) {
                    return Err(invalid("materialized entry became a link or reparse point"));
                }
                if kind == 0o040000 && metadata.is_dir() {
                    pending.push(name.clone());
                    vec![]
                } else if kind == 0o100000 && metadata.is_file() {
                    if metadata.len() != expected.size {
                        return Err(invalid("materialized file size changed"));
                    }
                    total = total
                        .checked_add(metadata.len())
                        .ok_or_else(|| invalid("materialization size overflow"))?;
                    if total > 512 * 1024 * 1024 {
                        return Err(invalid("materialization byte limit exceeded"));
                    }
                    let mut bytes = vec![];
                    fs::File::open(child.path())
                        .map_err(|_| invalid("cannot read materialized file"))?
                        .take(expected.size + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| invalid("cannot read materialized contents"))?;
                    bytes
                } else {
                    return Err(invalid("materialized entry type changed"));
                }
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if kind != 0o120000 && metadata.mode() & 0o7777 != expected.mode & 0o7777 {
                    return Err(invalid("materialized entry mode changed"));
                }
            }
            if bytes.len() as u64 != expected.size || hash(&bytes) != expected.digest {
                return Err(invalid("materialized contents changed"));
            }
            if !expected.implicit_directory {
                logical.insert(name.clone(), (expected.mode, bytes));
            }
            found.insert(name, ());
            if found.len() > 100_000 {
                return Err(invalid("materialization entry limit exceeded"));
            }
        }
    }
    if !partial && found.len() != baseline.len() {
        return Err(invalid("materialized entries are missing"));
    }
    Ok(snapshots::tree_digest(&logical))
}

pub(super) fn cleanup(root: &Path, baseline: &BTreeMap<String, MaterializedEntry>) -> Result<()> {
    check(root, baseline, true)?;
    if !root.exists() {
        return Ok(());
    }
    for (name, entry) in baseline.iter().rev() {
        validate_name(name)?;
        let path = root.join(name);
        let result = if entry.mode & 0o170000 == 0o040000 {
            fs::remove_dir(&path)
        } else {
            #[cfg(windows)]
            if fs::symlink_metadata(&path).is_ok_and(|m| {
                use std::os::windows::fs::FileTypeExt;
                m.file_type().is_symlink_dir()
            }) {
                fs::remove_dir(&path)
            } else {
                fs::remove_file(&path)
            }
            #[cfg(not(windows))]
            fs::remove_file(&path)
        };
        if let Err(e) = result
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(invalid("cannot remove owned materialized entry"));
        }
    }
    if let Err(e) = fs::remove_dir(root)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(invalid("cannot remove owned materialization"));
    }
    sync_dir(
        root.parent()
            .ok_or_else(|| invalid("materialization has no parent"))?,
    )
}
pub(super) fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| invalid("cannot flush materialization directory"))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))
            .map_err(|_| invalid("cannot preserve exported mode"))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}
fn reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn hash(bytes: &[u8]) -> String {
    utils::hex(&Sha256::digest(bytes))
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::Integrity, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::{ZipWriter, write::SimpleFileOptions};
    pub(in crate::core) fn archive(entries: &[(&str, u32, &str)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, mode, bytes) in entries {
            let options = SimpleFileOptions::default().unix_permissions(*mode);
            match mode & 0o170000 {
                0o120000 => zip.add_symlink(*name, *bytes, options).unwrap(),
                0o040000 => zip.add_directory(*name, options).unwrap(),
                _ => {
                    zip.start_file(*name, options).unwrap();
                    zip.write_all(bytes.as_bytes()).unwrap();
                }
            }
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn unsafe_archives_fail_even_without_optional_verification() {
        let temp = tempfile::tempdir().unwrap();
        let sentinel = temp.path().join("outside");
        fs::write(&sentinel, "preserve").unwrap();
        for entries in [
            vec![("../outside", 0o100644, "bad")],
            vec![("/outside", 0o100644, "bad")],
            vec![("C:/outside", 0o100644, "bad")],
            vec![("nested/.GIT", 0o100644, "bad")],
            vec![("a", 0o120777, "../outside")],
            vec![("a", 0o120777, "b"), ("b", 0o120777, "a")],
            vec![("a", 0o100644, "file"), ("a/b", 0o100644, "bad")],
            vec![("a", 0o120777, "b"), ("a/child", 0o100644, "bad")],
            vec![
                ("a/up", 0o120777, ".."),
                ("escape", 0o120777, "a/up/../outside"),
            ],
        ] {
            assert!(
                prepare(temp.path(), &archive(&entries), "unchecked", false).is_err(),
                "{entries:?}"
            );
            assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preserve");
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
        }
    }
    #[test]
    fn extracted_trees_are_independent_and_cleanup_preserves_modified_or_extra_files() {
        let temp = tempfile::tempdir().unwrap();
        let bytes = archive(&[("nested/run", 0o100755, "original")]);
        let expected = snapshots::tree_digest(&snapshots::decode(&bytes, None).unwrap());
        let first = prepare(temp.path(), &bytes, &expected, true).unwrap();
        let second = prepare(temp.path(), &bytes, &expected, false).unwrap();
        assert_eq!(
            check(first.staging.path(), &first.entries, false).unwrap(),
            expected
        );
        fs::write(first.staging.path().join("nested/run"), "modified").unwrap();
        assert!(check(first.staging.path(), &first.entries, false).is_err());
        assert!(cleanup(first.staging.path(), &first.entries).is_err());
        assert_eq!(
            fs::read_to_string(second.staging.path().join("nested/run")).unwrap(),
            "original"
        );
        fs::write(second.staging.path().join("extra"), "preserve").unwrap();
        assert!(cleanup(second.staging.path(), &second.entries).is_err());
        assert!(second.staging.path().join("nested/run").exists());
        fs::remove_file(second.staging.path().join("extra")).unwrap();
        fs::remove_file(second.staging.path().join("nested/run")).unwrap();
        cleanup(second.staging.path(), &second.entries).unwrap();
        cleanup(second.staging.path(), &second.entries).unwrap();
    }
    #[test]
    fn fast_materialization_never_replaces_the_expected_source_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let original = archive(&[("file", 0o100644, "original")]);
        let edited = archive(&[("file", 0o100644, "modified")]);
        let expected = snapshots::tree_digest(&snapshots::decode(&original, None).unwrap());
        assert!(prepare(temp.path(), &edited, &expected, true).is_err());
        let fast = prepare(temp.path(), &edited, &expected, false).unwrap();
        assert_ne!(
            check(fast.staging.path(), &fast.entries, false).unwrap(),
            expected
        );
    }
    #[test]
    fn declared_byte_bounds_are_rejected_before_inflation() {
        let temp = tempfile::tempdir().unwrap();
        let mut bytes = archive(&[("file", 0o100644, "small")]);
        let central = bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
        bytes[central + 24..central + 28]
            .copy_from_slice(&(512u32 * 1024 * 1024 + 1).to_le_bytes());
        assert!(prepare(temp.path(), &bytes, "unchecked", false).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        let mut bytes = archive(&[("device", 0o100644, "")]);
        let central = bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
        // Unix device type in the central directory's external attributes.
        bytes[central + 38..central + 42].copy_from_slice(&(0o020644u32 << 16).to_le_bytes());
        assert!(prepare(temp.path(), &bytes, "unchecked", false).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[test]
    fn excessive_entries_fail_before_creating_staging() {
        let temp = tempfile::tempdir().unwrap();
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..100_001 {
            zip.add_directory(format!("d{i}"), SimpleFileOptions::default())
                .unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        assert!(prepare(temp.path(), &bytes, "unchecked", false).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[cfg(windows)]
    #[test]
    fn windows_directory_link_chains_use_declared_targets_not_creation_order() {
        let temp = tempfile::tempdir().unwrap();
        let bytes = archive(&[
            ("a", 0o120777, "z"),
            ("z", 0o120777, "dir"),
            ("dir/file", 0o100644, "contents"),
        ]);
        let expected = snapshots::tree_digest(&snapshots::decode(&bytes, None).unwrap());
        match prepare(temp.path(), &bytes, &expected, true) {
            Ok(tree) => {
                assert!(tree.staging.path().join("a").is_dir());
                assert_eq!(
                    fs::read_to_string(tree.staging.path().join("a/file")).unwrap(),
                    "contents"
                );
                cleanup(tree.staging.path(), &tree.entries).unwrap();
            }
            Err(error) => {
                assert_eq!(error.kind, ErrorKind::Compatibility);
                assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
            }
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_aliases_and_reserved_names_fail_without_publication() {
        let temp = tempfile::tempdir().unwrap();
        for entries in [
            vec![("A/file", 0o100644, "a"), ("a/other", 0o100644, "b")],
            vec![("A", 0o100644, "a"), ("a", 0o100644, "b")],
            vec![("NUL.txt", 0o100644, "bad")],
            vec![("a.", 0o100644, "bad")],
            vec![("file:stream", 0o100644, "bad")],
        ] {
            assert!(prepare(temp.path(), &archive(&entries), "unchecked", false).is_err());
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        }
    }
    #[cfg(unix)]
    #[test]
    fn unix_modes_and_relative_links_survive_and_checked_reuse_checks_modes() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let bytes = archive(&[("run", 0o100755, "#!/bin/sh"), ("link", 0o120777, "run")]);
        let expected = snapshots::tree_digest(&snapshots::decode(&bytes, None).unwrap());
        let tree = prepare(temp.path(), &bytes, &expected, true).unwrap();
        assert_eq!(
            fs::read_link(tree.staging.path().join("link")).unwrap(),
            Path::new("run")
        );
        fs::set_permissions(
            tree.staging.path().join("run"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(check(tree.staging.path(), &tree.entries, false).is_err());
    }
}
