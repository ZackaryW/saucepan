use super::{fs::staged_directory, path::relative_path, tree::TreeLimits};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Seek},
    path::Path,
};

/// Bounded ZIP extraction; no retention, identity, or provider semantics.
pub fn extract_zip(
    reader: impl Read + Seek,
    destination: &Path,
    selection: Option<&str>,
    limits: TreeLimits,
) -> io::Result<()> {
    if let Some(folder) = selection {
        relative_path(folder)?;
    }
    let mut archive = zip::ZipArchive::new(reader).map_err(io::Error::other)?;
    if archive.len() > limits.entries {
        return Err(io::Error::other("archive entry limit exceeded"));
    }
    staged_directory(destination, |stage| {
        let mut seen = BTreeSet::new();
        let mut files = BTreeSet::new();
        let mut spellings = BTreeMap::new();
        let mut bytes = 0u64;
        let mut selected = false;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(io::Error::other)?;
            if entry.enclosed_name().is_none() {
                return Err(io::Error::other("escaping archive path"));
            }
            let name = entry.name().trim_end_matches('/').to_owned();
            relative_path(&name)?;
            let folded = name.to_lowercase();
            let original_parts: Vec<_> = name.split('/').collect();
            for end in 1..=original_parts.len() {
                let prefix = original_parts[..end].join("/");
                if let Some(previous) = spellings.insert(prefix.to_lowercase(), prefix.clone())
                    && previous != prefix
                {
                    return Err(io::Error::other("archive case collision"));
                }
            }
            if !seen.insert(folded.clone()) {
                return Err(io::Error::other("archive path collision"));
            }
            let parts: Vec<_> = folded.split('/').collect();
            for end in 1..parts.len() {
                if files.contains(&parts[..end].join("/")) {
                    return Err(io::Error::other("file/directory collision"));
                }
            }
            if !entry.is_dir() {
                if seen.iter().any(|n| n.starts_with(&(folded.clone() + "/"))) {
                    return Err(io::Error::other("file/directory collision"));
                }
                files.insert(folded);
            }
            let mode = entry.unix_mode().unwrap_or(0);
            if mode & 0o170000 != 0 && mode & 0o170000 != 0o100000 && mode & 0o170000 != 0o040000 {
                return Err(io::Error::other(
                    "archive links/special entries unsupported",
                ));
            }
            let relative = match selection {
                Some(folder) if name == folder && entry.is_dir() => Some(String::new()),
                Some(folder) => name
                    .strip_prefix(&(folder.to_owned() + "/"))
                    .map(str::to_owned),
                None => Some(name.clone()),
            };
            let mut output = None;
            if let Some(relative) = relative {
                selected = true;
                if !relative.is_empty() {
                    let path = stage.join(relative_path(&relative)?);
                    if entry.is_dir() {
                        fs::create_dir_all(&path)?;
                    } else {
                        fs::create_dir_all(path.parent().unwrap())?;
                        output = Some((
                            fs::OpenOptions::new()
                                .create_new(true)
                                .write(true)
                                .open(&path)?,
                            path,
                        ));
                    }
                }
            }
            let mut buffer = [0u8; 65536];
            loop {
                let n = entry.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                bytes = bytes
                    .checked_add(n as u64)
                    .ok_or_else(|| io::Error::other("archive size overflow"))?;
                if bytes > limits.bytes {
                    return Err(io::Error::other("archive byte limit exceeded"));
                }
                if let Some((file, _)) = &mut output {
                    use io::Write;
                    file.write_all(&buffer[..n])?;
                }
            }
            #[cfg(unix)]
            if let Some((_, path)) = output {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    path,
                    fs::Permissions::from_mode(if mode & 0o111 != 0 { 0o755 } else { 0o644 }),
                )?;
            }
        }
        if selection.is_some() && !selected {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "archive folder missing",
            ));
        }
        Ok(())
    })
}
