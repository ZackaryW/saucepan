use crate::{
    core::{models::*, sources::GitRepository},
    utils,
};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub(in crate::core) struct Exported {
    pub bytes: Vec<u8>,
    pub tree_digest: String,
    pub lfs_objects: Vec<LfsObject>,
}
pub(in crate::core) struct PreparedExport {
    entries: Entries,
    pub lfs_objects: Vec<LfsObject>,
    pending_lfs: Vec<LfsObject>,
    pub submodules: Vec<SubmoduleExport>,
}
pub(in crate::core) struct SubmoduleExport {
    pub path: String,
    pub selection: String,
    pub commit: String,
    pub locator: String,
}
#[cfg(test)]
fn create(
    repository: &GitRepository,
    commit: &str,
    selection: &str,
    staging: &Path,
) -> Result<Exported> {
    prepare_export(repository, commit, selection, staging)?.finish(repository, commit, staging)
}
pub(in crate::core) fn prepare_export(
    repository: &GitRepository,
    commit: &str,
    selection: &str,
    staging: &Path,
) -> Result<PreparedExport> {
    let links = repository.gitlinks(commit, selection)?;
    let crossing = links.iter().find(|link| {
        link.path == selection
            || selection
                .strip_prefix(&link.path)
                .is_some_and(|rest| rest.starts_with('/'))
    });
    let exported_selection = crossing.map_or(selection, |link| link.path.as_str());
    if crossing.is_none() {
        repository.tree(commit, selection)?;
    }
    let raw = repository.archive(commit, exported_selection, staging)?;
    let entries = decode_inner(&raw, Some(exported_selection), true, false)?;
    let mut submodules = vec![];
    for link in &links {
        let (path, child_selection, included) = if crossing.is_some() {
            let all = decode_inner(&raw, Some("."), true, false)?;
            (
                ".".to_owned(),
                selection
                    .strip_prefix(&link.path)
                    .unwrap()
                    .trim_start_matches('/')
                    .to_owned(),
                all.contains_key(&link.path),
            )
        } else {
            let path = if selection == "." {
                link.path.clone()
            } else {
                link.path
                    .strip_prefix(&format!("{selection}/"))
                    .unwrap()
                    .to_owned()
            };
            let included = entries.contains_key(&path);
            (path, String::new(), included)
        };
        if included {
            submodules.push(SubmoduleExport {
                path,
                selection: if child_selection.is_empty() {
                    ".".into()
                } else {
                    child_selection
                },
                commit: link.commit.clone(),
                locator: repository.submodule_origin(commit, &link.path)?,
            });
        }
    }
    let mut lfs_objects = vec![];
    let mut total = 0u64;
    for (path, (mode, bytes)) in &entries {
        let pointer = if mode & 0o170000 == 0o100000 {
            pointer(path, bytes)?
        } else {
            None
        };
        total = total
            .checked_add(
                pointer
                    .as_ref()
                    .map_or(bytes.len() as u64, |object| object.size),
            )
            .ok_or_else(|| invalid("export size overflow"))?;
        if total > 512 * 1024 * 1024 {
            return Err(invalid("expanded LFS export byte limit exceeded"));
        }
        if let Some(pointer) = pointer {
            lfs_objects.push(pointer);
        }
    }
    if !lfs_objects.is_empty() {
        // Endpoint checks also run before reuse of an existing current ZIP.
        repository.lfs_download(commit, staging)?;
    }
    Ok(PreparedExport {
        entries,
        pending_lfs: lfs_objects.clone(),
        lfs_objects,
        submodules,
    })
}
impl PreparedExport {
    pub(in crate::core) fn finish(
        self,
        repository: &GitRepository,
        commit: &str,
        staging: &Path,
    ) -> Result<Exported> {
        self.finish_tree(repository, commit, staging, true)
    }
    pub(in crate::core) fn finish_nested(
        self,
        repository: &GitRepository,
        commit: &str,
        staging: &Path,
    ) -> Result<Exported> {
        self.finish_tree(repository, commit, staging, false)
    }
    fn finish_tree(
        mut self,
        repository: &GitRepository,
        commit: &str,
        staging: &Path,
        rooted: bool,
    ) -> Result<Exported> {
        if !self.submodules.is_empty() {
            return Err(Error::new(
                ErrorKind::Source,
                "required submodule content must be resolved before export",
            ));
        }
        if !self.pending_lfs.is_empty() {
            let download = repository.lfs_download(commit, staging)?;
            for object in &self.pending_lfs {
                self.entries
                    .get_mut(&object.path)
                    .expect("pointer belongs to this export")
                    .1 = download.object(object)?;
            }
        }
        if self.entries.len() > 100_000
            || self
                .entries
                .values()
                .map(|(_, bytes)| bytes.len() as u64)
                .sum::<u64>()
                > 512 * 1024 * 1024
        {
            return Err(invalid("complete export exceeds entry or byte limits"));
        }
        if rooted {
            for (path, (mode, bytes)) in &self.entries {
                if mode & 0o170000 == 0o120000 {
                    validate_link(path, bytes)?;
                }
            }
            validate_tree(&self.entries)?;
        }
        let mut exported = encode(self.entries)?;
        exported.lfs_objects = self.lfs_objects;
        Ok(exported)
    }
    pub(in crate::core) fn include(&mut self, path: &str, child: Exported) -> Result<()> {
        let entries = decode_inner(&child.bytes, None, true, false)?;
        for (name, value) in entries {
            let name = if path == "." {
                name
            } else {
                format!("{path}/{name}")
            };
            if self.entries.insert(name, value).is_some() {
                return Err(invalid("submodule export collides with parent content"));
            }
        }
        for mut object in child.lfs_objects {
            if path != "." {
                object.path = format!("{path}/{}", object.path);
            }
            self.lfs_objects.push(object);
        }
        self.lfs_objects.sort_by(|a, b| a.path.cmp(&b.path));
        if self.entries.len() > 100_000
            || self
                .entries
                .values()
                .map(|(_, bytes)| bytes.len() as u64)
                .sum::<u64>()
                > 512 * 1024 * 1024
        {
            return Err(invalid(
                "complete submodule export exceeds entry or byte limits",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
fn repack(raw: Vec<u8>, selection: &str) -> Result<Exported> {
    let entries = decode_inner(&raw, Some(selection), false, true)?;
    encode(entries)
}

/// Export strips Git administration; consuming an existing snapshot rejects it.
pub(in crate::core) fn decode(raw: &[u8], selection: Option<&str>) -> Result<Entries> {
    // An LFS object's actual bytes may themselves resemble a pointer. Only
    // committed Git blobs are interpreted as pointers, never expanded output.
    decode_inner(raw, selection, true, true)
}
pub(in crate::core) fn validate_lfs(raw: &[u8], objects: &[LfsObject]) -> Result<()> {
    if objects.is_empty() {
        return Ok(());
    }
    let entries = decode(raw, None)?;
    for object in objects {
        let (mode, bytes) = entries
            .get(&object.path)
            .ok_or_else(|| invalid("snapshot is missing a declared LFS object"))?;
        if mode & 0o170000 != 0o100000
            || bytes.len() as u64 != object.size
            || utils::hex(&Sha256::digest(bytes)) != object.oid
        {
            return Err(invalid(
                "snapshot LFS object does not match its declared SHA-256 and size",
            ));
        }
    }
    Ok(())
}
fn decode_inner(
    raw: &[u8],
    selection: Option<&str>,
    allow_pointers: bool,
    rooted: bool,
) -> Result<Entries> {
    if raw.len() > 64 * 1024 * 1024 {
        return Err(invalid("compressed archive size limit exceeded"));
    }
    let mut input =
        ZipArchive::new(Cursor::new(raw)).map_err(|_| invalid("invalid Git archive"))?;
    if input.len() > 100_000 {
        return Err(invalid("export entry limit exceeded"));
    }
    let mut entries = std::collections::BTreeMap::new();
    let mut total = 0u64;
    for i in 0..input.len() {
        let mut entry = input
            .by_index(i)
            .map_err(|_| invalid("invalid Git archive entry"))?;
        let full = std::str::from_utf8(entry.name_raw())
            .map_err(|_| invalid("exported path is not UTF-8"))?
            .to_owned();
        let name = match selection {
            None | Some(".") => full.as_str(),
            Some(selection) => match full.strip_prefix(&format!("{selection}/")) {
                Some(name) => name,
                None => continue,
            },
        };
        if name.is_empty() || name.split('/').any(|c| c.eq_ignore_ascii_case(".git")) {
            if selection.is_some() {
                continue;
            }
            return Err(invalid(
                "snapshot contains Git administration or an empty path",
            ));
        }
        let name = name.trim_end_matches('/');
        super::super::recipes::validate_selection(name)?;
        if name == "." {
            return Err(invalid("invalid exported root entry"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| invalid("export size overflow"))?;
        if total > 512 * 1024 * 1024 {
            return Err(invalid("export byte limit exceeded"));
        }
        let mode = entry
            .unix_mode()
            .unwrap_or(if entry.is_dir() { 0o40755 } else { 0o100644 });
        if !matches!(mode & 0o170000, 0o040000 | 0o100000 | 0o120000) {
            return Err(invalid("unsupported Git entry type"));
        }
        if entry.is_dir() != (mode & 0o170000 == 0o040000) {
            return Err(invalid("archive path and entry type disagree"));
        }
        let mut bytes = Vec::new();
        let entry_limit = entry.size().min(512 * 1024 * 1024) + 1;
        (&mut entry)
            .take(entry_limit)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid("cannot read Git archive entry"))?;
        if bytes.len() as u64 != entry.size() {
            return Err(invalid("Git archive entry size mismatch"));
        }
        if mode & 0o170000 == 0o040000 && !bytes.is_empty() {
            return Err(invalid("archive directory has contents"));
        }
        if !allow_pointers
            && (bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n")
                || bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\r\n"))
        {
            return Err(Error::new(
                ErrorKind::Source,
                "required Git LFS object must be resolved before export",
            ));
        }
        if rooted && mode & 0o170000 == 0o120000 {
            validate_link(name, &bytes)?;
        }
        if entries.insert(name.to_owned(), (mode, bytes)).is_some() {
            return Err(invalid("duplicate exported path"));
        }
    }
    if rooted {
        validate_tree(&entries)?;
    }
    Ok(entries)
}

pub(in crate::core) fn tree_digest(entries: &Entries) -> String {
    let mut digest = Sha256::new();
    for (name, (mode, bytes)) in entries {
        digest.update(utils::frame(
            b"saucepan/tree-entry/v1",
            &[name.as_bytes(), &mode.to_be_bytes(), bytes],
        ));
    }
    utils::hex(&digest.finalize())
}

fn encode(entries: Entries) -> Result<Exported> {
    let tree_digest = tree_digest(&entries);
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, (mode, bytes)) in entries {
        let options = SimpleFileOptions::default().unix_permissions(mode);
        if mode & 0o170000 == 0o040000 {
            output
                .add_directory(&name, options)
                .map_err(|_| invalid("cannot encode archive directory"))?;
        } else if mode & 0o170000 == 0o120000 {
            output
                .add_symlink(
                    &name,
                    std::str::from_utf8(&bytes).map_err(|_| invalid("non-UTF-8 link target"))?,
                    options,
                )
                .map_err(|_| invalid("cannot encode archive link"))?;
        } else {
            output
                .start_file(&name, options)
                .map_err(|_| invalid("cannot encode archive file"))?;
            output
                .write_all(&bytes)
                .map_err(|_| invalid("cannot encode archive contents"))?;
        }
    }
    let bytes = output
        .finish()
        .map_err(|_| invalid("cannot finish archive"))?
        .into_inner();
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(invalid("compressed archive size limit exceeded"));
    }
    Ok(Exported {
        bytes,
        tree_digest,
        lfs_objects: vec![],
    })
}
fn pointer(path: &str, bytes: &[u8]) -> Result<Option<LfsObject>> {
    if !bytes.starts_with(b"version https://git-lfs.github.com/spec/") {
        return Ok(None);
    }
    if bytes.len() > 1024 {
        return Err(invalid("Git LFS pointer exceeds size limit"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("invalid Git LFS pointer"))?;
    let mut lines = text.lines();
    if lines.next() != Some("version https://git-lfs.github.com/spec/v1") {
        return Err(Error::new(
            ErrorKind::Compatibility,
            "unsupported Git LFS pointer version",
        ));
    }
    let oid = lines
        .next()
        .and_then(|line| line.strip_prefix("oid sha256:"))
        .filter(|oid| {
            oid.len() == 64
                && oid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or_else(|| invalid("invalid or extended Git LFS object declaration"))?;
    let size = lines
        .next()
        .and_then(|line| line.strip_prefix("size "))
        .filter(|size| !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|size| size.parse::<u64>().ok())
        .filter(|size| *size <= 512 * 1024 * 1024)
        .ok_or_else(|| invalid("invalid Git LFS size declaration"))?;
    if lines.next().is_some() {
        return Err(invalid(
            "extended or malformed Git LFS pointer is unsupported",
        ));
    }
    Ok(Some(LfsObject {
        path: path.into(),
        oid: oid.into(),
        size,
    }))
}
fn validate_link(name: &str, bytes: &[u8]) -> Result<()> {
    let target = std::str::from_utf8(bytes).map_err(|_| invalid("non-UTF-8 link target"))?;
    if target.is_empty()
        || target.starts_with('/')
        || target.contains(['\\', ':'])
        || target.chars().any(char::is_control)
    {
        return Err(invalid("unsafe exported link target"));
    }
    let mut depth = name.split('/').count() - 1;
    for part in target.split('/') {
        match part {
            ".." => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("exported link escapes artifact"))?;
            }
            "" | "." => {}
            part if part.eq_ignore_ascii_case(".git") => {
                return Err(invalid("link targets Git administration"));
            }
            _ => depth += 1,
        }
    }
    Ok(())
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::Integrity, message)
}

pub(in crate::core) type Entries = std::collections::BTreeMap<String, (u32, Vec<u8>)>;
fn validate_tree(entries: &Entries) -> Result<()> {
    for (name, (mode, bytes)) in entries {
        let mut parent = name.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if entries
                .get(prefix)
                .is_some_and(|(mode, _)| mode & 0o170000 != 0o040000)
            {
                return Err(invalid("exported file or link is used as a directory"));
            }
            parent = prefix;
        }
        if mode & 0o170000 == 0o120000 {
            let target =
                std::str::from_utf8(bytes).map_err(|_| invalid("non-UTF-8 link target"))?;
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
                            .ok_or_else(|| invalid("link chain escapes artifact"))?;
                        continue;
                    }
                    _ => resolved.push(part),
                }
                if let Some((mode, target)) = entries.get(&resolved.join("/")) {
                    if mode & 0o170000 == 0o120000 {
                        hops += 1;
                        if hops > 64 {
                            return Err(invalid("exported link cycle or depth limit"));
                        }
                        resolved.pop();
                        let target = std::str::from_utf8(target)
                            .map_err(|_| invalid("non-UTF-8 link target"))?;
                        for part in target.split('/').rev() {
                            pending.push_front(part.into());
                        }
                    } else if mode & 0o170000 != 0o040000 && !pending.is_empty() {
                        return Err(invalid("link traverses a non-directory"));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sources::{
        git_tests::{fixture, git},
        identify,
    };

    #[test]
    fn lfs_declarations_and_cached_payloads_are_checked_independently_of_optional_hashes() {
        let oid = utils::hex(&Sha256::digest(b"good"));
        let object = LfsObject {
            path: "data".into(),
            oid: oid.clone(),
            size: 4,
        };
        assert!(
            validate_lfs(
                &archive(&[("data", 0o100644, "good")]),
                std::slice::from_ref(&object)
            )
            .is_ok()
        );
        assert!(
            validate_lfs(
                &archive(&[("data", 0o100644, "evil")]),
                std::slice::from_ref(&object)
            )
            .is_err()
        );
        assert!(validate_lfs(&archive(&[]), std::slice::from_ref(&object)).is_err());
        for suffix in [
            format!("oid sha256:{oid}\nsize 536870913\n"),
            format!("oid sha256:{oid}\nsize 4\nsize 4\n"),
            format!("ext-0-run sha256:{oid}\noid sha256:{oid}\nsize 4\n"),
            format!("oid sha256:{oid}\nsize +4\n"),
        ] {
            assert!(
                pointer(
                    "data",
                    format!("version https://git-lfs.github.com/spec/v1\n{suffix}").as_bytes()
                )
                .is_err()
            );
        }
        assert_eq!(
            pointer(
                "data",
                format!(
                    "version https://git-lfs.github.com/spec/v1\r\noid sha256:{oid}\r\nsize 4\r\n"
                )
                .as_bytes()
            )
            .unwrap(),
            Some(object)
        );
    }

    #[test]
    fn native_lfs_expands_selected_objects_and_rejects_missing_corrupt_and_repointed_data() {
        let remote = fixture();
        let content = b"actual large file bytes\n";
        let oid = utils::hex(&Sha256::digest(content));
        let object = remote
            .path()
            .join(".git/lfs/objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(&oid);
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, content).unwrap();
        std::fs::create_dir(remote.path().join("pkg")).unwrap();
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
            content.len()
        );
        std::fs::write(remote.path().join("pkg/large.bin"), &pointer).unwrap();
        // An unresolved sibling must not be acquired by a subtree export.
        std::fs::write(
            remote.path().join("missing.bin"),
            pointer.replace(&oid, &"f".repeat(64)),
        )
        .unwrap();
        std::fs::write(
            remote.path().join(".gitattributes"),
            "pkg/large.bin filter=arbitrary\n",
        )
        .unwrap();
        git(remote.path(), &["add", "."]);
        git(remote.path(), &["commit", "-m", "LFS fixture"]);
        let identity = identify(
            &Recipe::git(remote.path().to_string_lossy()).source,
            remote.path(),
        )
        .unwrap();
        let stage = tempfile::tempdir().unwrap();
        let repo = GitRepository::clone_into(&stage.path().join("repo.git"), &identity).unwrap();
        let commit = repo.resolve(&Revision::DefaultBranch).unwrap();
        let exported = create(&repo, &commit, "pkg", stage.path()).unwrap();
        let entries = decode(&exported.bytes, None).unwrap();
        assert_eq!(entries["large.bin"].1, content);
        assert_eq!(
            exported.lfs_objects,
            vec![LfsObject {
                path: "large.bin".into(),
                oid: oid.clone(),
                size: content.len() as u64
            }]
        );
        assert!(create(&repo, &commit, ".", stage.path()).is_err());
        std::fs::write(&object, b"corrupted object bytes\n!").unwrap();
        assert!(create(&repo, &commit, "pkg", stage.path()).is_err());
        std::fs::remove_file(&object).unwrap();
        assert!(create(&repo, &commit, "pkg", stage.path()).is_err());
        std::fs::write(&object, content).unwrap();
        std::fs::write(
            remote.path().join(".lfsconfig"),
            "[lfs]\nurl = https://denied.invalid/objects\n",
        )
        .unwrap();
        git(remote.path(), &["add", ".lfsconfig"]);
        git(
            remote.path(),
            &["commit", "-m", "unauthorized LFS endpoint"],
        );
        let commit = repo.resolve(&Revision::DefaultBranch).unwrap();
        assert!(
            matches!(create(&repo, &commit, "pkg", stage.path()), Err(e) if e.kind == ErrorKind::Authority)
        );
    }
    fn archive(entries: &[(&str, u32, &str)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, mode, contents) in entries {
            let options = SimpleFileOptions::default().unix_permissions(*mode);
            if mode & 0o170000 == 0o120000 {
                zip.add_symlink(*name, *contents, options).unwrap();
            } else if mode & 0o170000 == 0o040000 {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(contents.as_bytes()).unwrap();
            }
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn strips_git_administration_at_every_depth_and_preserves_modes_and_safe_links() {
        let raw = archive(&[
            ("pkg/.git/config", 0o100644, "private"),
            ("pkg/nested/.GIT", 0o100644, "gitdir: private"),
            ("pkg/.gitignore", 0o100644, "build/"),
            ("pkg/run", 0o100755, "#!/bin/sh\n"),
            ("pkg/link", 0o120777, "run"),
            ("sibling", 0o100644, "exclude"),
        ]);
        let exported = repack(raw, "pkg").unwrap();
        let mut zip = ZipArchive::new(Cursor::new(exported.bytes)).unwrap();
        assert_eq!(zip.len(), 3);
        assert!(zip.by_name(".gitignore").is_ok());
        assert_eq!(
            zip.by_name("run").unwrap().unix_mode().unwrap() & 0o777,
            0o755
        );
        assert_eq!(
            zip.by_name("link").unwrap().unix_mode().unwrap() & 0o170000,
            0o120000
        );
    }
    #[test]
    fn rejects_path_traversal_link_chains_cycles_and_lfs_placeholders() {
        for entries in [
            vec![("../escape", 0o100644, "bad")],
            vec![("link", 0o120777, "../escape")],
            vec![("a", 0o120777, "b"), ("b", 0o120777, "a")],
            vec![("link", 0o120777, "safe"), ("link/child", 0o100644, "bad")],
            vec![
                ("dir/up", 0o120777, ".."),
                ("escape", 0o120777, "dir/up/../outside"),
            ],
            vec![(
                "large",
                0o100644,
                "version https://git-lfs.github.com/spec/v1\n",
            )],
        ] {
            assert!(repack(archive(&entries), ".").is_err(), "{entries:?}");
        }
    }
}
