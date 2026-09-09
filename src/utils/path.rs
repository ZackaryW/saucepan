use std::{io, path::PathBuf};

/// Parse a nonempty, slash-separated relative path without traversal or native prefixes.
///
/// This is lexical validation only. Filesystem collisions and symlinks must be
/// checked by the caller before accessing a destination.
pub fn relative_path(value: impl AsRef<str>) -> io::Result<PathBuf> {
    let value = value.as_ref();
    if value.contains(['\\', ':', '\0'])
        || value.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a slash-separated relative path without empty, dot, or prefixed components",
        ));
    }
    Ok(value.split('/').collect())
}

/// Validate a logical relative path for creation on this host. Filesystem-specific
/// collisions still require checking at creation time in a private directory.
pub fn native_relative_path(value: impl AsRef<str>) -> io::Result<PathBuf> {
    let value = value.as_ref();
    let path = relative_path(value)?;
    #[cfg(windows)]
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or_default().to_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        if reserved
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| c < ' ' || "<>\"|?*".contains(c))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unrepresentable Windows path",
            ));
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nested_paths_spaces_and_unicode() {
        assert_eq!(
            relative_path("folder/é file.txt").unwrap(),
            PathBuf::from("folder").join("é file.txt")
        );
        assert_eq!(
            relative_path(String::from("file.txt")).unwrap(),
            PathBuf::from("file.txt")
        );
    }

    #[test]
    fn rejects_empty_and_ambiguous_components() {
        for value in ["", ".", "..", "a/../b", "a/./b", "a//b", "a/"] {
            assert_eq!(
                relative_path(value).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{value:?}"
            );
        }
    }

    #[test]
    fn rejects_posix_and_windows_escapes_on_every_host() {
        for value in [
            "/etc/passwd",
            "//host/file",
            r"\root",
            r"a\..\b",
            "C:/file",
            "C:file",
            r"\\host\share",
            "a/file:stream",
            "a/\0b",
        ] {
            assert_eq!(
                relative_path(value).unwrap_err().kind(),
                io::ErrorKind::InvalidInput,
                "{value:?}"
            );
        }
    }
}
