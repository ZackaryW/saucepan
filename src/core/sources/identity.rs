use crate::{core::models::*, utils};
use sha2::{Digest, Sha256};
use std::path::Path;
use url::Url;

/// Restore an authenticated canonical record without interpreting it again as
/// user transport syntax (notably github: and scp: encodings).
pub(in crate::core) fn recorded_git(id: &str, origin: &str) -> Result<SourceIdentity> {
    if hash(Backend::Git, origin) != id {
        return Err(Error::new(
            ErrorKind::Integrity,
            "recorded source identity changed",
        ));
    }
    Ok(SourceIdentity {
        schema_version: SCHEMA_VERSION,
        backend: Backend::Git,
        id: id.into(),
        origin: origin.into(),
        locator: origin.into(),
    })
}

/// No network, configuration lookup, or store mutation. Filesystem origins use
/// canonical absolute paths; the coordinator authenticates context first.
pub(in crate::core) fn identify(locator: &SourceLocator, base: &Path) -> Result<SourceIdentity> {
    if locator.backend != Backend::Git {
        return Err(invalid("unsupported artifact backend"));
    }
    let text = locator.origin.as_str();
    if text.is_empty() || text.starts_with('-') || text.chars().any(char::is_control) {
        return Err(invalid("invalid Git origin"));
    }
    let origin = if Path::new(text).is_absolute()
        || text.starts_with("./")
        || text.starts_with("../")
    {
        file_origin(&base.join(text))?
    } else if text.contains("://") {
        let url = Url::parse(text).map_err(|_| invalid("invalid Git URL"))?;
        if url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (!url.username().is_empty() && url.scheme() != "ssh")
        {
            return Err(invalid(
                "use native Git authentication; URL credentials and query parameters are not supported",
            ));
        }
        if url.scheme() == "file" {
            if url.host_str().is_some_and(|host| host != "localhost") {
                return Err(invalid("file Git origins must be local paths"));
            }
            file_origin(
                &url.to_file_path()
                    .map_err(|_| invalid("invalid filesystem Git URL"))?,
            )?
        } else {
            if !matches!(url.scheme(), "https" | "http" | "ssh" | "git") || url.host_str().is_none()
            {
                return Err(invalid("unsupported Git transport"));
            }
            if url
                .host_str()
                .is_some_and(|h| h.eq_ignore_ascii_case("github.com"))
                && ((url.scheme() == "https" && url.port_or_known_default() == Some(443))
                    || (url.scheme() == "ssh"
                        && url.port().is_none_or(|p| p == 22)
                        && matches!(url.username(), "" | "git")))
            {
                github(url.path().trim_start_matches('/'))?
            } else {
                // Keep arbitrary repository path case, suffixes, transport and
                // SSH login selectors: custom hosts need not alias them.
                url.to_string()
            }
        }
    } else if let Some((authority, path)) = text.split_once(':') {
        if authority.contains(['/', '\\'])
            || path.starts_with(':')
            || path.is_empty()
            || path.chars().any(char::is_whitespace)
        {
            return Err(invalid("invalid SSH Git origin"));
        }
        let (user, host) = authority.split_once('@').unwrap_or(("", authority));
        if host.is_empty()
            || host.contains('@')
            || user.contains(':')
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
            || !user
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return Err(invalid("invalid SSH Git origin"));
        }
        if host.eq_ignore_ascii_case("github.com") && matches!(user, "" | "git") {
            github(path)?
        } else {
            // scp relative-home paths are not equivalent to ssh absolute paths.
            format!("scp:{user}@{}:{path}", host.to_ascii_lowercase())
        }
    } else {
        github(text)?
    };
    let id = hash(Backend::Git, &origin);
    Ok(SourceIdentity {
        schema_version: 1,
        backend: Backend::Git,
        id,
        origin,
        locator: text.into(),
    })
}
fn file_origin(path: &Path) -> Result<String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| invalid("filesystem Git origin cannot be resolved"))?;
    if !canonical.is_dir() {
        return Err(invalid("filesystem Git origin must be a directory"));
    }
    Url::from_file_path(canonical)
        .map(|u| u.to_string())
        .map_err(|_| invalid("filesystem Git origin cannot be represented"))
}
fn github(path: &str) -> Result<String> {
    let path = path.trim_end_matches('/');
    let (owner, repo) = path
        .split_once('/')
        .ok_or_else(|| invalid("GitHub source must identify owner/repository"))?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if owner.is_empty()
        || repo.is_empty()
        || matches!(repo, "." | "..")
        || !owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !repo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(invalid("invalid GitHub repository identity"));
    }
    Ok(format!(
        "github:{}/{}",
        owner.to_ascii_lowercase(),
        repo.to_ascii_lowercase()
    ))
}
fn hash(backend: Backend, origin: &str) -> String {
    let backend: &[u8] = match backend {
        Backend::Git => b"git",
        Backend::Http => b"http",
        Backend::Local => b"local",
    };
    utils::hex(&Sha256::digest(utils::frame(
        b"saucepan/source/v1",
        &[backend, origin.as_bytes()],
    )))
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::Config, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(origin: &str) -> Result<SourceIdentity> {
        identify(&Recipe::git(origin).source, Path::new("."))
    }
    #[test]
    fn canonical_records_are_not_reparsed_as_transport_locators() {
        for locator in [
            "Owner/Repo",
            "git@example.test:Case/Repo.git",
            "https://example.test/Case/Repo.git",
        ] {
            let identity = id(locator).unwrap();
            let restored = recorded_git(&identity.id, &identity.origin).unwrap();
            assert_eq!(restored.id, identity.id);
            assert_eq!(restored.origin, identity.origin);
            // Probe native Git with remote-looking identities without fetching
            // anything. Local-file-only fixtures cannot catch this boundary.
            let temp = tempfile::tempdir().unwrap();
            for args in [
                vec!["init", "--bare", "--template="],
                vec!["remote", "add", "origin", locator],
            ] {
                let output = std::process::Command::new("git")
                    .arg("-C")
                    .arg(temp.path())
                    .args(args)
                    .output()
                    .unwrap();
                assert!(output.status.success());
            }
            super::super::GitRepository::open(temp.path(), &restored).unwrap();
            assert!(
                matches!(recorded_git(&identity.id, &format!("{}-changed", identity.origin)), Err(e) if e.kind == ErrorKind::Integrity)
            );
        }
    }
    #[test]
    fn github_aliases_share_identity_without_erasing_locator() {
        let expected = id("Owner/Repo").unwrap();
        for spelling in [
            "owner/repo",
            "https://github.com/Owner/Repo.git",
            "git@github.com:owner/repo.git",
            "ssh://git@github.com/owner/repo",
            "ssh://git@github.com:22/owner/repo",
            "https://GITHUB.com:443/owner/repo/",
        ] {
            let actual = id(spelling).unwrap();
            assert_eq!(actual.id, expected.id, "{spelling}");
            assert_eq!(actual.locator, spelling);
        }
        assert_eq!(expected.id.len(), 64);
    }
    #[test]
    fn arbitrary_origins_and_backend_encodings_remain_distinct() {
        assert_ne!(
            id("https://example.test/Repo").unwrap().id,
            id("https://example.test/repo").unwrap().id
        );
        assert_ne!(id("a/b--c").unwrap().id, id("a--b/c").unwrap().id);
        assert_ne!(
            id("git@example.test:repo").unwrap().id,
            id("ssh://git@example.test/repo").unwrap().id
        );
        assert_ne!(hash(Backend::Git, "same"), hash(Backend::Http, "same"));
        assert_ne!(
            id("ssh://git@github.com:2222/owner/repo").unwrap().id,
            id("owner/repo").unwrap().id
        );
    }
    #[test]
    fn credentials_and_ambiguous_transports_are_rejected_without_echoing_them() {
        for origin in [
            "https://secret@example.test/repo",
            "ssh://git:secret@example.test/repo",
            "https://example.test/repo?token=secret",
            "https://example.test/repo#secret",
            "ext::secret",
            "owner/repo/subdir",
        ] {
            let error = id(origin).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Config);
            assert!(!error.message().contains("secret"));
        }
    }
    #[test]
    fn filesystem_path_and_file_url_share_canonical_identity() {
        let home = tempfile::tempdir().unwrap();
        let path = std::fs::canonicalize(home.path()).unwrap();
        let plain = id(path.to_str().unwrap()).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        assert_eq!(plain.id, id(url.as_str()).unwrap().id);
    }
}
