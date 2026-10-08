use super::path::{plain_metadata, relative_path};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    path::Path,
};

#[derive(Clone, Copy)]
pub struct TreeLimits {
    pub entries: usize,
    pub bytes: u64,
}
impl Default for TreeLimits {
    fn default() -> Self {
        Self {
            entries: 1_000_000,
            bytes: 16 * 1024 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub digest: Option<String>,
    pub executable: bool,
}
pub type TreeSummary = BTreeMap<String, Entry>;

/// Local observation seam for inspection tests; contains no store identities.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum IoEvent {
    Directory,
    Open,
    Bytes(usize),
}

pub fn digest(reader: impl Read) -> io::Result<String> {
    let mut hash = Sha256::new();
    let mut reader = reader;
    let mut buffer = [0u8; 65536];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn executable(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// Stable source and new private destination are caller responsibilities.
pub fn copy_tree(source: &Path, destination: &Path, limits: TreeLimits) -> io::Result<TreeSummary> {
    if !plain_metadata(source)?.is_dir() {
        return Err(io::Error::other("source must be a directory"));
    }
    fs::create_dir(destination)?;
    let mut summary = BTreeMap::new();
    let mut bytes = 0;
    visit(
        source,
        source,
        destination,
        limits,
        &mut bytes,
        &mut summary,
        true,
        &mut |_| {},
    )?;
    Ok(summary)
}
pub fn inspect(root: &Path, limits: TreeLimits) -> io::Result<TreeSummary> {
    inspect_observed(root, limits, &mut |_| {})
}
pub(crate) fn inspect_observed(
    root: &Path,
    limits: TreeLimits,
    observer: &mut dyn FnMut(IoEvent),
) -> io::Result<TreeSummary> {
    if !plain_metadata(root)?.is_dir() {
        return Err(io::Error::other("not a directory"));
    }
    let mut summary = BTreeMap::new();
    let mut bytes = 0;
    visit(
        root,
        root,
        root,
        limits,
        &mut bytes,
        &mut summary,
        false,
        observer,
    )?;
    Ok(summary)
}
#[allow(clippy::too_many_arguments)]
fn visit(
    root: &Path,
    source: &Path,
    destination: &Path,
    limits: TreeLimits,
    bytes: &mut u64,
    summary: &mut TreeSummary,
    copy: bool,
    observer: &mut dyn FnMut(IoEvent),
) -> io::Result<()> {
    observer(IoEvent::Directory);
    let mut entries = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    let mut names = BTreeSet::new();
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::other("non-UTF8 filename"))?;
        relative_path(&name)?;
        if !names.insert(name.to_lowercase()) {
            return Err(io::Error::other("case-colliding filenames"));
        }
        if summary.len() >= limits.entries {
            return Err(io::Error::other("entry limit exceeded"));
        }
        let path = entry.path();
        let meta = plain_metadata(&path)?;
        let key = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF8 path"))?
            .replace('\\', "/");
        let target = destination.join(&name);
        if meta.is_dir() {
            summary.insert(
                key,
                Entry {
                    digest: None,
                    executable: false,
                },
            );
            if copy {
                fs::create_dir(&target)?;
            }
            visit(root, &path, &target, limits, bytes, summary, copy, observer)?;
        } else {
            let mut input = fs::File::open(&path)?;
            observer(IoEvent::Open);
            let mut output = if copy {
                Some(
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)?,
                )
            } else {
                None
            };
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            let mut length = 0;
            loop {
                let n = input.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                length += n as u64;
                observer(IoEvent::Bytes(n));
                *bytes = bytes
                    .checked_add(n as u64)
                    .ok_or_else(|| io::Error::other("byte overflow"))?;
                if *bytes > limits.bytes {
                    return Err(io::Error::other("byte limit exceeded"));
                }
                hash.update(&buffer[..n]);
                if let Some(out) = &mut output {
                    out.write_all(&buffer[..n])?;
                }
            }
            let after = plain_metadata(&path)?;
            if length != meta.len()
                || after.len() != meta.len()
                || after.modified()? != meta.modified()?
            {
                return Err(io::Error::other("source changed during read"));
            }
            if copy {
                fs::set_permissions(&target, meta.permissions())?;
            }
            summary.insert(
                key,
                Entry {
                    digest: Some(format!("{:x}", hash.finalize())),
                    executable: executable(&meta),
                },
            );
        }
    }
    Ok(())
}
