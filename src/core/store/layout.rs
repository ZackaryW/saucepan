use crate::core::models::{Error, ErrorKind, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Clone)]
pub(super) struct Layout {
    root: PathBuf,
}

impl Layout {
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn at_test_root(root: &Path) -> Result<Self> {
        let absolute = std::path::absolute(root)
            .map_err(|_| Error::new(ErrorKind::Config, "invalid test store path"))?;
        let parent = absolute.parent().ok_or_else(|| {
            Error::new(ErrorKind::Config, "test store requires a parent directory")
        })?;
        let parent = fs::canonicalize(parent)
            .map_err(|_| Error::new(ErrorKind::Config, "test store parent must exist"))?;
        let name = absolute
            .file_name()
            .ok_or_else(|| Error::new(ErrorKind::Config, "test store requires a directory name"))?;
        let layout = Self {
            root: parent.join(name),
        };
        reject_link(&layout.root)?;
        Ok(layout)
    }
    pub(super) fn for_user() -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::new(ErrorKind::Authority, "OS user home is unavailable"))?;
        Self::under(&home)
    }
    pub(super) fn under(home: &Path) -> Result<Self> {
        let parent = fs::canonicalize(home)
            .map_err(|_| Error::new(ErrorKind::Authority, "user home cannot be resolved"))?;
        Ok(Self {
            root: parent.join(".saucepan"),
        })
    }
    pub(super) fn root(&self) -> &Path {
        &self.root
    }
    pub(super) fn path(&self, relative: &Path) -> Result<PathBuf> {
        if relative.as_os_str().is_empty()
            || relative.components().any(|c| match c {
                Component::Normal(name) => name.to_str().is_none_or(|name| {
                    name.ends_with(['.', ' '])
                        || name.contains([':', '\\'])
                        || name.chars().any(char::is_control)
                        || reserved_name(name)
                }),
                _ => true,
            })
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "invalid managed relative path",
            ));
        }
        // Both existing directories and the final entry must be non-reparse
        // owned paths. This is containment under the trusted-user threat model,
        // not protection against an adversarial same-user rename race.
        reject_link(&self.root)?;
        let mut path = self.root.clone();
        for c in relative.components() {
            path.push(c.as_os_str());
            reject_link(&path)?;
        }
        Ok(path)
    }
    pub(super) fn initialize_root(&self) -> Result<()> {
        fs::create_dir(&self.root).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::new(
                    ErrorKind::Conflict,
                    "store already exists; explicit recovery is required for partial state",
                )
            } else {
                io_error("cannot create user store", e)
            }
        })?;
        private_dir(&self.root)?;
        for dir in [
            "bin",
            "sources",
            "materializations",
            "transactions",
            "locks",
        ] {
            let path = self.path(Path::new(dir))?;
            fs::create_dir(&path).map_err(|e| io_error("cannot create store directory", e))?;
            private_dir(&path)?;
        }
        Ok(())
    }
    pub(super) fn read(&self, relative: &Path, limit: usize) -> Result<Vec<u8>> {
        let path = self.path(relative)?;
        let mut file =
            fs::File::open(&path).map_err(|e| io_error("cannot read managed file", e))?;
        if !file
            .metadata()
            .map_err(|e| io_error("cannot inspect managed file", e))?
            .is_file()
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "managed entry is not a regular file",
            ));
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io_error("cannot read managed file", e))?;
        if bytes.len() > limit {
            return Err(Error::new(
                ErrorKind::Integrity,
                "managed file exceeds size limit",
            ));
        }
        Ok(bytes)
    }
    pub(super) fn create_directory(&self, relative: &Path) -> Result<()> {
        let path = self.path(relative)?;
        fs::create_dir(&path).map_err(|e| io_error("cannot create owned directory", e))?;
        private_dir(&path)
    }
    /// Publish an immutable file without replacing an existing entry.
    pub(super) fn create_file(&self, relative: &Path, bytes: &[u8]) -> Result<()> {
        let path = self.path(relative)?;
        let parent = path
            .parent()
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "invalid owned file"))?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| io_error("cannot stage immutable file", e))?;
        staged
            .write_all(bytes)
            .and_then(|_| staged.as_file().sync_all())
            .map_err(|e| io_error("cannot flush immutable file", e))?;
        staged
            .persist_noclobber(&path)
            .map_err(|e| io_error("cannot publish immutable file", e.error))?;
        sync_directory(parent)
    }
    /// Only individual journal-owned files may be removed here; never recurse.
    pub(super) fn remove_file(&self, relative: &Path) -> Result<()> {
        let path = self.path(relative)?;
        match fs::remove_file(&path) {
            Ok(()) => sync_directory(path.parent().expect("managed file has a parent")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error("cannot remove owned file", e)),
        }
    }
    pub(super) fn replace(&self, relative: &Path, bytes: &[u8]) -> Result<()> {
        let path = self.path(relative)?;
        let parent = path
            .parent()
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "invalid managed destination"))?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| io_error("cannot stage managed file", e))?;
        staged
            .write_all(bytes)
            .map_err(|e| io_error("cannot stage managed contents", e))?;
        staged
            .as_file()
            .sync_all()
            .map_err(|e| io_error("cannot flush managed contents", e))?;
        // tempfile's platform implementation performs replacement on Windows too.
        staged
            .persist(&path)
            .map_err(|e| io_error("cannot publish managed file", e.error))?;
        sync_directory(parent)
    }
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| io_error("cannot flush managed directory", e))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || (stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}
fn private_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| io_error("cannot restrict store permissions", e))?;
    }
    #[cfg(windows)]
    {
        reject_link(path)?;
        // Use normal permissions inherited from the OS user profile. App
        // scoping belongs to the encrypted index, not a custom Windows ACL.
    }
    Ok(())
}
pub(super) fn reject_link(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(windows)]
            let link = {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let link = meta.file_type().is_symlink();
            if link {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "managed path contains a symbolic link or reparse point",
                ));
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error("cannot inspect managed path", e)),
    }
}

/// Canonicalize an explicit external destination without traversing links. A
/// missing suffix is allowed for recovery of an already-removed mirror.
pub(super) fn external_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute()
        || path.file_name().is_none()
        || path.components().any(|part| match part {
            Component::ParentDir | Component::CurDir => true,
            Component::Normal(name) => name.to_str().is_none_or(|name| {
                name.ends_with(['.', ' '])
                    || name.contains([':', '\\'])
                    || name.chars().any(char::is_control)
                    || reserved_name(name)
            }),
            _ => false,
        })
    {
        return Err(Error::new(
            ErrorKind::Config,
            "mirror destination must be a normal absolute path",
        ));
    }
    let ancestors: Vec<_> = path.ancestors().collect();
    for ancestor in ancestors.iter().rev() {
        reject_link(ancestor)?;
    }
    let mut existing = path;
    let mut suffix = vec![];
    while !existing.exists() {
        suffix.push(
            existing
                .file_name()
                .ok_or_else(|| Error::new(ErrorKind::Config, "invalid mirror destination"))?,
        );
        existing = existing
            .parent()
            .ok_or_else(|| Error::new(ErrorKind::Config, "invalid mirror destination"))?;
    }
    let mut resolved = fs::canonicalize(existing)
        .map_err(|_| Error::new(ErrorKind::Config, "cannot resolve mirror destination"))?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}
pub(super) fn io_error(context: &str, error: std::io::Error) -> Error {
    Error::new(ErrorKind::Internal, format!("{context}: {}", error.kind()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_escape_and_preserves_unrecognized_root() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        for path in [
            "../outside",
            "/absolute",
            "a/../outside",
            "NUL",
            "a/CON.txt",
            "COM1",
            "a:stream",
            "trailing.",
            "trailing ",
        ] {
            assert!(layout.path(Path::new(path)).is_err());
        }
        layout.initialize_root().unwrap();
        fs::write(layout.root().join("keep"), "original").unwrap();
        assert_eq!(
            layout.initialize_root().unwrap_err().kind,
            ErrorKind::Conflict
        );
        assert_eq!(
            fs::read_to_string(layout.root().join("keep")).unwrap(),
            "original"
        );
    }
    #[test]
    fn replaces_atomically_and_bounds_reads() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        layout.replace(Path::new("index.json.enc"), b"old").unwrap();
        layout.replace(Path::new("index.json.enc"), b"new").unwrap();
        assert_eq!(layout.read(Path::new("index.json.enc"), 3).unwrap(), b"new");
        assert_eq!(
            layout
                .read(Path::new("index.json.enc"), 2)
                .unwrap_err()
                .kind,
            ErrorKind::Integrity
        );
    }
    #[cfg(windows)]
    #[test]
    fn junctioned_parent_never_changes_external_files() {
        let home = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        fs::write(outside.path().join("keep"), b"original").unwrap();
        let junction = layout.root().join("junction");
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:SAUCEPAN_JUNCTION_TEST_LINK -Target $env:SAUCEPAN_JUNCTION_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("SAUCEPAN_JUNCTION_TEST_LINK", &junction)
            .env("SAUCEPAN_JUNCTION_TEST_TARGET", outside.path())
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let replace = layout.replace(Path::new("junction/keep"), b"no");
        let remove = layout.remove_file(Path::new("junction/keep"));
        // Remove only the fixture reparse entry, never recurse through it.
        fs::remove_dir(&junction).unwrap();
        assert_eq!(replace.unwrap_err().kind, ErrorKind::Integrity);
        assert_eq!(remove.unwrap_err().kind, ErrorKind::Integrity);
        assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"original");
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_destination_is_not_followed() {
        let home = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        layout.initialize_root().unwrap();
        std::os::unix::fs::symlink(outside.path(), layout.root().join("link")).unwrap();
        assert!(layout.replace(Path::new("link/out"), b"no").is_err());
        assert!(!outside.path().join("out").exists());
    }
}
