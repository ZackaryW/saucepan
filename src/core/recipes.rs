use super::models::{
    Backend, EXPORT_VERSION, Error, ErrorKind, Recipe, Result, Revision, SCHEMA_VERSION,
};

pub(super) fn validate(recipe: &Recipe) -> Result<()> {
    if recipe.schema_version != SCHEMA_VERSION || recipe.export.version != EXPORT_VERSION {
        return Err(Error::new(
            ErrorKind::Compatibility,
            "unsupported recipe or export version",
        ));
    }
    if recipe.source.backend != Backend::Git {
        return Err(Error::new(
            ErrorKind::Config,
            "unsupported artifact backend",
        ));
    }
    let origin = &recipe.source.origin;
    if origin.is_empty() || origin.starts_with('-') || origin.chars().any(char::is_control) {
        return Err(Error::new(ErrorKind::Config, "invalid Git origin"));
    }
    // Credentials in URL authority must never enter persisted recipe/provenance.
    if origin.contains("://") {
        let url = url::Url::parse(origin)
            .map_err(|_| Error::new(ErrorKind::Config, "invalid Git URL"))?;
        if url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (!url.username().is_empty() && url.scheme() != "ssh")
        {
            return Err(Error::new(
                ErrorKind::Config,
                "use native Git credentials, not URL credentials",
            ));
        }
    }
    validate_selection(&recipe.export.subdirectory)?;
    match &recipe.revision {
        Revision::DefaultBranch => {}
        Revision::Commit(commit) => {
            if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::new(
                    ErrorKind::Config,
                    "commit pin must be a complete Git object ID",
                ));
            }
        }
        Revision::Branch(name) | Revision::Tag(name) => {
            if name.is_empty()
                || name.starts_with('-')
                || name.starts_with('/')
                || name.ends_with('/')
                || name.ends_with('.')
                || name.contains("..")
                || name.contains("@{")
                || name.contains("//")
                || name == "@"
                || name
                    .split('/')
                    .any(|c| c.starts_with('.') || c.ends_with(".lock"))
                || name
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c))
            {
                return Err(Error::new(
                    ErrorKind::Config,
                    "invalid Git revision selector",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn stream(
    recipe: &Recipe,
    source_id: &str,
) -> Result<(String, super::models::StreamDescriptor)> {
    use sha2::{Digest, Sha256};
    validate(recipe)?;
    let revision = match &recipe.revision {
        Revision::Commit(id) => Revision::Commit(id.to_ascii_lowercase()),
        other => other.clone(),
    };
    let descriptor = super::models::StreamDescriptor {
        source_id: source_id.into(),
        revision,
        export: recipe.export.clone(),
    };
    let bytes = serde_json::to_vec(&descriptor)
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode recipe stream"))?;
    let id = crate::utils::hex(&Sha256::digest(crate::utils::frame(
        b"saucepan/stream/v1",
        &[&bytes],
    )));
    Ok((id, descriptor))
}

pub(super) fn validate_selection(path: &str) -> Result<()> {
    if path == "." {
        return Ok(());
    }
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|c| c.is_empty() || c == "." || c == ".." || c.eq_ignore_ascii_case(".git"))
    {
        return Err(Error::new(
            ErrorKind::Config,
            "selection must be a normalized repository-relative directory",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_selections_and_non_git_before_effects() {
        for path in [
            "", "/tmp", "../a", "a/../b", ".git", "a/.GIT/b", "a\\b", "C:/a", "a//b",
        ] {
            let mut recipe = Recipe::git("owner/repo");
            recipe.export.subdirectory = path.into();
            assert!(validate(&recipe).is_err(), "{path}");
        }
        let mut recipe = Recipe::git("owner/repo");
        recipe.source.backend = Backend::Http;
        assert_eq!(validate(&recipe).unwrap_err().kind, ErrorKind::Config);
    }
    #[test]
    fn accepts_root_subtree_and_full_pins() {
        let mut recipe = Recipe::git("owner/repo");
        validate(&recipe).unwrap();
        recipe.export.subdirectory = "packages/tool".into();
        recipe.revision = Revision::Commit("a".repeat(40));
        validate(&recipe).unwrap();
        recipe.revision = Revision::Commit("abc".into());
        assert!(validate(&recipe).is_err());
    }
}
