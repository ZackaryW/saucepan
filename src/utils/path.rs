use std::{
    io,
    path::{Path, PathBuf},
};

/// Resolve a new destination under an existing parent and reject either direction
/// of overlap with existing source data. Does not create or reserve anything.
pub fn disjoint_destination(source: &Path, destination: &Path) -> io::Result<PathBuf> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "source and destination must be disjoint",
        )
    };
    let name = match destination.components().next_back() {
        Some(std::path::Component::Normal(name)) => name,
        _ => return Err(invalid()),
    };
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let destination = parent.canonicalize()?.join(name);
    let source = source.canonicalize()?;
    if destination.starts_with(&source) || source.starts_with(&destination) {
        return Err(invalid());
    }
    match destination.symlink_metadata() {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination occupied",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(destination),
        Err(error) => Err(error),
    }
}

/// Strict portable relative names. Existing filesystem components require separate checks.
pub fn relative_path(value: &str) -> io::Result<PathBuf> {
    let bad = || io::Error::new(io::ErrorKind::InvalidInput, "invalid relative path");
    if value.is_empty() || value.contains(['\\', ':', '\0']) {
        return Err(bad());
    }
    let mut out = PathBuf::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(bad());
        }
        // Portable names prevent metadata aliasing when transferring across hosts.
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if part.ends_with(['.', ' '])
            || part.chars().any(|c| c < ' ' || "<>\"|?*".contains(c))
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || ["COM", "LPT"].iter().any(|p| {
                stem.strip_prefix(p).is_some_and(|n| {
                    matches!(
                        n,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        {
            return Err(bad());
        }
        out.push(part);
    }
    Ok(out)
}

pub fn plain_metadata(path: &Path) -> io::Result<std::fs::Metadata> {
    let meta = path.symlink_metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other("reparse points are unsupported"));
        }
    }
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err(io::Error::other("links and special files are unsupported"));
    }
    Ok(meta)
}

/// Walk existing components beneath a trusted root, rejecting filesystem links.
pub fn beneath(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let relative = relative_path(relative)?;
    plain_metadata(root)?;
    let mut result = root.to_path_buf();
    for part in relative.components() {
        result.push(part);
        match plain_metadata(&result) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}
