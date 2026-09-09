use super::*;
use std::collections::BTreeMap;

/// Committed gitlink metadata, before source identity or authority is resolved.
pub(in crate::core) struct Gitlink {
    pub path: String,
    pub commit: String,
}

impl GitRepository {
    pub(in crate::core) fn gitlinks(&self, commit: &str, selection: &str) -> Result<Vec<Gitlink>> {
        let tree = self.tree(commit, ".")?;
        let mut selected = vec![];
        for entry in tree
            .split(|b| *b == 0)
            .filter(|entry| entry.starts_with(b"160000 "))
        {
            let separator = entry
                .iter()
                .position(|b| *b == b'\t')
                .ok_or_else(|| integrity("invalid Git submodule tree entry"))?;
            let (metadata, path) = (&entry[..separator], &entry[separator + 1..]);
            let path =
                std::str::from_utf8(path).map_err(|_| source("submodule path is not UTF-8"))?;
            if path
                .split('/')
                .any(|part| part.eq_ignore_ascii_case(".git"))
            {
                continue;
            }
            if selection != "."
                && path != selection
                && !path
                    .strip_prefix(selection)
                    .is_some_and(|rest| rest.starts_with('/'))
                && !selection
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('/'))
            {
                continue;
            }
            crate::core::recipes::validate_selection(path)?;
            let oid = metadata.rsplit(|b| *b == b' ').next().unwrap_or_default();
            let oid =
                std::str::from_utf8(oid).map_err(|_| integrity("invalid submodule commit"))?;
            if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(integrity("invalid submodule commit"));
            }
            selected.push((path.to_owned(), oid.to_owned()));
        }
        Ok(selected
            .into_iter()
            .map(|(path, commit)| Gitlink { path, commit })
            .collect())
    }
    pub(in crate::core) fn submodule_origin(&self, commit: &str, path: &str) -> Result<String> {
        self.verify_origin()?;
        let config = self
            .command(&[
                "config",
                "--no-includes",
                "--blob",
                &format!("{commit}:.gitmodules"),
                "--null",
                "--list",
            ])
            .map_err(|_| {
                source("required committed submodule configuration is missing or invalid")
            })?;
        let mut modules: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for entry in config.split(|b| *b == 0).filter(|entry| !entry.is_empty()) {
            let text = std::str::from_utf8(entry)
                .map_err(|_| source("submodule configuration is not UTF-8"))?;
            let Some((key, value)) = text.split_once('\n') else {
                continue;
            };
            let Some(key) = key.strip_prefix("submodule.") else {
                continue;
            };
            let Some((name, field)) = key.rsplit_once('.') else {
                continue;
            };
            if matches!(field, "path" | "url")
                && modules
                    .entry(name.into())
                    .or_default()
                    .insert(field.into(), value.into())
                    .is_some()
            {
                return Err(source("ambiguous committed submodule configuration"));
            }
        }
        let origin = self.text(&["remote", "get-url", "origin"])?;
        let mut matches = modules
            .values()
            .filter(|module| module.get("path").is_some_and(|value| value == path));
        let locator = matches
            .next()
            .and_then(|module| module.get("url"))
            .ok_or_else(|| source("required submodule URL is missing"))?;
        if matches.next().is_some() {
            return Err(source("ambiguous committed submodule path"));
        }
        relative_origin(&origin, locator)
    }
}

fn relative_origin(parent: &str, child: &str) -> Result<String> {
    if child.is_empty() || child.chars().any(char::is_control) || child.contains('\\') {
        return Err(source("invalid submodule source URL"));
    }
    if !child.starts_with("./") && !child.starts_with("../") {
        return Ok(child.into());
    }
    if let Ok(mut url) = url::Url::parse(parent)
        && (url.has_host() || url.scheme() == "file")
    {
        // Git interprets relative URLs against the repository as a directory.
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
        return url
            .join(child)
            .map(|url| url.to_string())
            .map_err(|_| source("invalid relative submodule URL"));
    }
    // Native scp-like SSH spelling: host:path/repository.git + ../sibling.git.
    if let Some((host, path)) = parent.split_once(':') {
        let mut components: Vec<_> = path.trim_end_matches('/').split('/').collect();
        for part in child.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    components
                        .pop()
                        .ok_or_else(|| source("relative submodule URL escapes its origin"))?;
                }
                part => components.push(part),
            }
        }
        return Ok(format!("{host}:{}", components.join("/")));
    }
    Err(source(
        "relative submodule URL requires a verified absolute parent origin",
    ))
}
