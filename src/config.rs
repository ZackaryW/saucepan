use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::error::ConfigError;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub jq: Option<String>,
    pub local: Option<LocalSource>,
    pub github: Option<GithubSource>,
    pub customgit: Option<CustomGitSource>,
    /// Authentication for fetching registered central indexes when there is
    /// no "primary" source to inherit from — i.e. everywhere except the
    /// install/update manifest resolution chain. During install/update, the
    /// central-index chain link reuses the *primary* target's own
    /// `GitFetchOptions` (binary/token/ssl_key), which is a sound default
    /// because a primary fetch is already in flight there. Standalone bucket
    /// commands (`bucket add`/`refresh`, `search`, `cat bucket`) have no such
    /// primary fetch to borrow from, so they use this section instead.
    /// Entirely optional: an index needing no auth (the common case — public
    /// repos, or local paths/`file://` URLs which never use it) needs no
    /// `[index]` section, and these commands fall back to plain,
    /// unauthenticated `git`, exactly as before this field existed.
    pub index: Option<IndexSource>,
}

impl Config {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("saucepan.toml");
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| ConfigError(format!("cannot read {}: {e}", path.display())))?;
        let cfg = toml::from_str(&contents)
            .map_err(|e| ConfigError(format!("invalid saucepan.toml: {e}")))?;
        Ok(cfg)
    }

    pub fn jq_bin(&self) -> &str {
        self.jq.as_deref().unwrap_or("jq")
    }

    pub fn local_enabled(&self) -> bool {
        self.local.is_some()
    }

    pub fn github_enabled(&self) -> bool {
        self.github.is_some()
    }

    pub fn customgit_enabled(&self) -> bool {
        self.customgit.is_some()
    }

    /// The Git binary to use for standalone central-index fetches
    /// (`bucket add`/`refresh`, `search`, `cat bucket`). Defaults to `git`
    /// when no `[index]` section is configured.
    pub fn index_binary(&self) -> GitBinary {
        self.index
            .as_ref()
            .map(|i| i.binary.clone())
            .unwrap_or_default()
    }

    /// The token to use for standalone central-index fetches, if configured.
    pub fn index_token(&self) -> Option<&str> {
        self.index.as_ref().and_then(|i| i.token.as_deref())
    }

    /// The SSL key to use for standalone central-index fetches, if configured.
    pub fn index_ssl_key(&self) -> Option<&str> {
        self.index.as_ref().and_then(|i| i.ssl_key.as_deref())
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct LocalSource {}

#[derive(Debug, Deserialize)]
pub struct GithubSource {
    #[serde(default = "default_git_binary")]
    pub binary: GitBinary,
    pub token: Option<String>,
    pub ssl_key: Option<String>,
    pub manifest: Option<String>,
}

impl GithubSource {
    pub fn manifest_name(&self) -> &str {
        self.manifest.as_deref().unwrap_or("sauce.json")
    }
}

#[derive(Debug, Deserialize)]
pub struct CustomGitSource {
    pub url: String,
    #[serde(default = "default_git_binary")]
    pub binary: GitBinary,
    pub token: Option<String>,
    pub ssl_key: Option<String>,
    pub manifest: Option<String>,
}

impl CustomGitSource {
    pub fn manifest_name(&self) -> &str {
        self.manifest.as_deref().unwrap_or("sauce.json")
    }
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GitBinary {
    #[default]
    Git,
    Gh,
}

fn default_git_binary() -> GitBinary {
    GitBinary::Git
}

/// Authentication used to fetch a registered central index when no primary
/// source's `GitFetchOptions` is available to reuse. See `Config::index`.
#[derive(Debug, Deserialize, Default)]
pub struct IndexSource {
    #[serde(default = "default_git_binary")]
    pub binary: GitBinary,
    pub token: Option<String>,
    pub ssl_key: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_toml(dir: &TempDir, content: &str) -> std::path::PathBuf {
        let p = dir.path().join("saucepan.toml");
        fs::write(&p, content).unwrap();
        dir.path().to_path_buf()
    }

    #[test]
    fn missing_file_errors() {
        let dir = TempDir::new().unwrap();
        let err = Config::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("saucepan.toml"));
    }

    #[test]
    fn empty_toml_is_valid() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "");
        let cfg = Config::load(&root).unwrap();
        assert!(!cfg.local_enabled());
        assert!(!cfg.github_enabled());
        assert!(!cfg.customgit_enabled());
    }

    #[test]
    fn local_section_enables_local() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[local]\n");
        let cfg = Config::load(&root).unwrap();
        assert!(cfg.local_enabled());
        assert!(!cfg.github_enabled());
    }

    #[test]
    fn jq_bin_defaults_to_jq() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.jq_bin(), "jq");
    }

    #[test]
    fn jq_bin_uses_configured_path() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "jq = \"/usr/local/bin/jq\"\n");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.jq_bin(), "/usr/local/bin/jq");
    }

    #[test]
    fn github_defaults_binary_to_git() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[github]\n");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.github.unwrap().binary, GitBinary::Git);
    }

    #[test]
    fn github_accepts_gh_binary() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[github]\nbinary = \"gh\"\n");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.github.unwrap().binary, GitBinary::Gh);
    }

    #[test]
    fn github_manifest_defaults_to_sauce_json() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[github]\n");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.github.unwrap().manifest_name(), "sauce.json");
    }

    #[test]
    fn github_manifest_custom_name() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[github]\nmanifest = \"pkg.json\"\n");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.github.unwrap().manifest_name(), "pkg.json");
    }

    #[test]
    fn customgit_requires_url() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[customgit]\n");
        let err = Config::load(&root).unwrap_err();
        assert!(err.to_string().contains("invalid saucepan.toml"));
    }

    #[test]
    fn customgit_manifest_defaults_to_sauce_json() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "[customgit]\nurl = \"https://git.example.com\"\n");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.customgit.unwrap().manifest_name(), "sauce.json");
    }

    #[test]
    fn customgit_manifest_custom_name() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(
            &dir,
            "[customgit]\nurl = \"https://git.example.com\"\nmanifest = \"meta.json\"\n",
        );
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.customgit.unwrap().manifest_name(), "meta.json");
    }

    #[test]
    fn index_auth_defaults_to_unauthenticated_git_when_unconfigured() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(&dir, "");
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.index_binary(), GitBinary::Git);
        assert!(cfg.index_token().is_none());
        assert!(cfg.index_ssl_key().is_none());
    }

    #[test]
    fn index_section_configures_standalone_index_fetch_auth() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(
            &dir,
            "[index]\nbinary = \"gh\"\ntoken = \"secret\"\nssl_key = \"/keys/id\"\n",
        );
        let cfg = Config::load(&root).unwrap();
        assert_eq!(cfg.index_binary(), GitBinary::Gh);
        assert_eq!(cfg.index_token(), Some("secret"));
        assert_eq!(cfg.index_ssl_key(), Some("/keys/id"));
    }

    #[test]
    fn index_section_is_independent_of_github_and_customgit() {
        let dir = TempDir::new().unwrap();
        let root = write_toml(
            &dir,
            "[github]\ntoken = \"github-secret\"\n[index]\ntoken = \"index-secret\"\n",
        );
        let cfg = Config::load(&root).unwrap();
        assert_eq!(
            cfg.github.as_ref().unwrap().token.as_deref(),
            Some("github-secret")
        );
        assert_eq!(cfg.index_token(), Some("index-secret"));
    }
}
