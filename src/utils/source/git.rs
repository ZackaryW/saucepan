//! Git mechanics only. Callers own source identity, locks, freshness, and policy.
use crate::utils::{
    fs::staged_directory,
    links,
    path::{plain_metadata, relative_path},
    process,
    tree::{self, TreeLimits},
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
    process::Command,
    time::Duration,
};

pub struct Git {
    pub timeout: Duration,
    pub limits: TreeLimits,
}
impl Default for Git {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            limits: TreeLimits::default(),
        }
    }
}
#[derive(Default)]
pub struct Export {
    pub dependencies: BTreeMap<String, String>,
    pub executables: BTreeSet<String>,
}
#[derive(Default)]
struct Inventory {
    result: Export,
    entries: BTreeMap<String, links::Entry>,
    bytes: u64,
}

// Git for Windows does not accept Rust's verbatim canonical path spelling in
// every command (notably clone destinations and LFS configuration).
fn git_path(path: &Path) -> std::path::PathBuf {
    #[cfg(windows)]
    if let Some(value) = path.to_str() {
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return format!("//{}", rest.replace('\\', "/")).into();
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return rest.replace('\\', "/").into();
        }
    }
    path.to_owned()
}

/// Normalize supported URLs and existing local origins. No shell expansion.
pub fn normalize(origin: &str) -> Result<String> {
    ensure!(
        !origin.is_empty() && !origin.starts_with('-') && !origin.contains(['\n', '\r', '\0']),
        "invalid Git origin"
    );
    if Path::new(origin).is_absolute() || Path::new(origin).exists() {
        let path = fs::canonicalize(origin)?;
        return url::Url::from_file_path(path)
            .map(String::from)
            .map_err(|_| anyhow::anyhow!("invalid local origin"));
    }
    let expanded = if origin.starts_with("git@") && !origin.contains("://") {
        let (host, path) = origin.split_once(':').context("invalid SSH origin")?;
        format!("ssh://{host}/{path}")
    } else {
        origin.to_owned()
    };
    let mut url = url::Url::parse(&expanded)?;
    ensure!(
        matches!(url.scheme(), "https" | "http" | "ssh" | "git" | "file"),
        "unsupported Git transport"
    );
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "Git origin cannot have query or fragment"
    );
    if url.scheme() == "file" {
        let path = url
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("invalid file URL"))?;
        if path.exists() {
            return normalize(path.to_str().context("non-UTF-8 origin")?);
        }
        return Ok(url.into());
    }
    ensure!(url.host_str().is_some(), "missing Git host");
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url.into())
}

impl Git {
    fn command(&self, repo: &Path) -> Command {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(git_path(repo))
            .args([
                "-c",
                "core.longpaths=true",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "maintenance.auto=false",
                "-c",
                "protocol.allow=never",
                "-c",
                "protocol.file.allow=always",
                "-c",
                "protocol.https.allow=always",
                "-c",
                "protocol.http.allow=always",
                "-c",
                "protocol.ssh.allow=always",
                "-c",
                "protocol.git.allow=always",
                "-c",
                "http.lowSpeedLimit=1",
                "-c",
                "http.lowSpeedTime=30",
            ])
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_LFS_SKIP_SMUDGE", "1");
        for name in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_CONFIG",
            "GIT_CONFIG_COUNT",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        ] {
            command.env_remove(name);
        }
        command
    }
    fn run(&self, repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
        let mut command = self.command(repo);
        command.args(args);
        let output = process::run(command, self.timeout, 64 * 1024 * 1024)?;
        ensure!(
            output.status.success(),
            "Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        ensure!(!output.truncated, "Git capture exceeded limit");
        Ok(output.stdout)
    }
    fn text(&self, repo: &Path, args: &[&str]) -> Result<String> {
        Ok(String::from_utf8(self.run(repo, args)?)?.trim().into())
    }

    pub fn repository(&self, origin: &str, repo: &Path) -> Result<()> {
        let origin = normalize(origin)?;
        if !repo.try_exists()? {
            staged_directory(repo, |stage| {
                let mut command = self.command(repo.parent().unwrap());
                command
                    .args([
                        "clone",
                        "--bare",
                        "--no-hardlinks",
                        "--origin",
                        "origin",
                        "--",
                        &origin,
                    ])
                    .arg(git_path(stage));
                let output = process::run(command, self.timeout, 65536)?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other(format!(
                        "Git clone failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )))
                }
            })?;
        }
        ensure!(plain_metadata(repo)?.is_dir(), "repository missing");
        for configured in [
            self.text(repo, &["config", "--get-all", "remote.origin.url"])?,
            self.text(repo, &["remote", "get-url", "--all", "origin"])?,
        ] {
            ensure!(normalize(&configured)? == origin, "Git origin mismatch");
        }
        Ok(())
    }

    /// Fetch a tracked ref, or ensure an exact pinned commit is locally available.
    pub fn resolve(&self, repo: &Path, requested: &str, pinned: bool) -> Result<String> {
        ensure!(
            !requested.is_empty()
                && !requested.starts_with('-')
                && !requested.contains(['\n', '\r', '\0']),
            "invalid Git selector"
        );
        if pinned {
            ensure!(
                matches!(requested.len(), 40 | 64)
                    && requested.bytes().all(|b| b.is_ascii_hexdigit()),
                "pinned commit must be a full object ID"
            );
        }
        let expression = format!("{requested}^{{commit}}");
        if pinned
            && let Ok(commit) = self.text(
                repo,
                &["rev-parse", "--verify", "--end-of-options", &expression],
            )
            && self.run(repo, &["cat-file", "-e", &expression]).is_ok()
        {
            return Ok(commit);
        }
        self.run(
            repo,
            &["fetch", "--no-tags", "--force", "origin", requested],
        )?;
        let commit = self.text(repo, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"])?;
        ensure!(
            !pinned || commit.eq_ignore_ascii_case(requested),
            "pinned commit mismatch"
        );
        Ok(commit)
    }

    pub fn export(
        &self,
        origin: &str,
        repo: &Path,
        commit: &str,
        cache: &Path,
        destination: &Path,
    ) -> Result<Export> {
        ensure!(
            matches!(commit.len(), 40 | 64) && commit.bytes().all(|b| b.is_ascii_hexdigit()),
            "export requires an exact commit ID"
        );
        let scratch = tempfile::tempdir_in(cache)?;
        let input = scratch.path().join("input");
        fs::create_dir(&input)?;
        let mut inventory = Inventory::default();
        self.inventory(
            (origin, repo),
            commit,
            (&input, cache),
            "",
            &mut inventory,
            0,
        )?;
        let mut result = Export {
            dependencies: inventory.result.dependencies.clone(),
            executables: BTreeSet::new(),
        };
        staged_directory(destination, |stage| {
            let mut seen = BTreeSet::new();
            links::expand(
                &inventory.entries,
                links::Limits {
                    hops: 64,
                    entries: self.limits.entries,
                    bytes: self.limits.bytes,
                },
                |name, original, entry| {
                    if !seen.insert(name.to_lowercase()) {
                        return Err(std::io::Error::other("Git case collision"));
                    }
                    let dest = stage.join(relative_path(name)?);
                    if matches!(entry, links::Entry::Directory) {
                        return fs::create_dir(&dest);
                    }
                    fs::copy(input.join(relative_path(original)?), &dest)?;
                    if inventory.result.executables.contains(original) {
                        result.executables.insert(name.into());
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
                        }
                    }
                    Ok(())
                },
            )
        })?;
        Ok(result)
    }

    fn inventory(
        &self,
        source: (&str, &Path),
        commit: &str,
        paths: (&Path, &Path),
        prefix: &str,
        inventory: &mut Inventory,
        depth: usize,
    ) -> Result<()> {
        let (origin, repo) = source;
        let (root, cache) = paths;
        ensure!(depth <= 32, "submodule nesting exceeded");
        for entry in self
            .run(repo, &["ls-tree", "-rtzl", "--full-tree", commit])?
            .split(|b| *b == 0)
            .filter(|e| !e.is_empty())
        {
            let split = entry
                .iter()
                .position(|b| *b == b'\t')
                .context("invalid Git tree")?;
            let header: Vec<_> = std::str::from_utf8(&entry[..split])?
                .split_whitespace()
                .collect();
            ensure!(header.len() == 4, "invalid Git tree header");
            let name = std::str::from_utf8(&entry[split + 1..])?;
            ensure!(
                !name.split('/').any(|p| p.eq_ignore_ascii_case(".git")),
                "reserved Git directory"
            );
            let relative = relative_path(name)?;
            let logical = if prefix.is_empty() {
                name.into()
            } else {
                format!("{prefix}/{name}")
            };
            ensure!(
                inventory.entries.len() < self.limits.entries
                    && !inventory.entries.contains_key(&logical),
                "Git entry limit or duplicate"
            );
            let dest = root.join(relative);
            match header[0] {
                "040000" | "160000" => {
                    inventory
                        .entries
                        .insert(logical.clone(), links::Entry::Directory);
                    fs::create_dir_all(&dest)?;
                    if header[0] == "040000" {
                        continue;
                    }
                    let child = self.submodule_origin(origin, repo, commit, name, cache)?;
                    let child_cache = cache
                        .join("submodules")
                        .join(tree::digest(child.as_bytes())?);
                    fs::create_dir_all(&child_cache)?;
                    let child_repo = child_cache.join("repo");
                    self.repository(&child, &child_repo)?;
                    let resolved = self.resolve(&child_repo, header[2], true)?;
                    inventory
                        .result
                        .dependencies
                        .insert(logical.clone(), resolved.clone());
                    self.inventory(
                        (&child, &child_repo),
                        &resolved,
                        (&dest, &child_cache),
                        &logical,
                        inventory,
                        depth + 1,
                    )?;
                }
                "100644" | "100755" | "120000" => {
                    let size: u64 = header[3].parse()?;
                    inventory.bytes = inventory
                        .bytes
                        .checked_add(size)
                        .filter(|b| *b <= self.limits.bytes)
                        .context("Git byte limit exceeded")?;
                    if header[0] == "120000" {
                        ensure!(size <= 65536, "Git link too large");
                        inventory.entries.insert(
                            logical,
                            links::Entry::Link(self.run(repo, &["cat-file", "blob", header[2]])?),
                        );
                        continue;
                    }
                    let file = fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(&dest)?;
                    let mut command = self.command(repo);
                    command.args(["cat-file", "blob", header[2]]);
                    let status = process::run_to_file(command, file, self.timeout, 65536)?;
                    ensure!(
                        status.status.success() && fs::metadata(&dest)?.len() == size,
                        "Git blob unavailable"
                    );
                    if let Some(oid) = self.expand_lfs(repo, commit, &dest)? {
                        inventory
                            .result
                            .dependencies
                            .insert(format!("lfs:{logical}"), oid);
                    }
                    let size_after = fs::metadata(&dest)?.len();
                    inventory.bytes = inventory
                        .bytes
                        .checked_sub(size)
                        .and_then(|b| b.checked_add(size_after))
                        .filter(|b| *b <= self.limits.bytes)
                        .context("Git byte limit exceeded")?;
                    if header[0] == "100755" {
                        inventory.result.executables.insert(logical.clone());
                    }
                    inventory
                        .entries
                        .insert(logical, links::Entry::File(size_after));
                }
                _ => bail!("unsupported Git mode"),
            }
        }
        Ok(())
    }

    fn submodule_origin(
        &self,
        origin: &str,
        repo: &Path,
        commit: &str,
        path: &str,
        cache: &Path,
    ) -> Result<String> {
        let mut config = tempfile::NamedTempFile::new_in(cache)?;
        config.write_all(&self.run(repo, &["show", &format!("{commit}:.gitmodules")])?)?;
        let config_native = git_path(config.path());
        let config_path = config_native.to_str().context("non-UTF-8 config path")?;
        let entries = self.run(
            repo,
            &[
                "config",
                "--no-includes",
                "--file",
                config_path,
                "--null",
                "--get-regexp",
                "^submodule\\..*\\.path$",
            ],
        )?;
        let mut result = None;
        for entry in entries.split(|b| *b == 0).filter(|e| !e.is_empty()) {
            let (key, value) = std::str::from_utf8(entry)?
                .split_once('\n')
                .context("invalid submodule config")?;
            if value != path {
                continue;
            }
            ensure!(result.is_none(), "ambiguous submodule path");
            let key = format!(
                "{}.url",
                key.strip_suffix(".path").context("invalid submodule key")?
            );
            let locator = self.text(
                repo,
                &[
                    "config",
                    "--no-includes",
                    "--file",
                    config_path,
                    "--get",
                    &key,
                ],
            )?;
            let locator = if locator.starts_with("./") || locator.starts_with("../") {
                url::Url::parse(&format!("{origin}/"))?
                    .join(&locator)?
                    .to_string()
            } else {
                locator
            };
            result = Some(normalize(&locator)?);
        }
        result.context("missing submodule origin")
    }

    fn expand_lfs(&self, repo: &Path, commit: &str, file: &Path) -> Result<Option<String>> {
        let mut head = Vec::new();
        fs::File::open(file)?.take(1025).read_to_end(&mut head)?;
        if !head.starts_with(b"version https://git-lfs.github.com/spec/v1") {
            return Ok(None);
        }
        let pointer = std::str::from_utf8(&head)?;
        ensure!(
            head.len() <= 1024
                && pointer.lines().count() == 3
                && pointer.lines().next() == Some("version https://git-lfs.github.com/spec/v1"),
            "invalid LFS pointer"
        );
        let oid = pointer
            .lines()
            .find_map(|l| l.strip_prefix("oid sha256:"))
            .context("missing LFS digest")?;
        ensure!(
            oid.len() == 64
                && oid
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "invalid LFS digest"
        );
        let size: u64 = pointer
            .lines()
            .find_map(|l| l.strip_prefix("size "))
            .context("missing LFS size")?
            .parse()?;
        ensure!(size <= self.limits.bytes, "LFS byte limit exceeded");
        let storage = git_path(&repo.join("lfs"));
        let object = storage
            .join("objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(oid);
        if !object.try_exists()? {
            self.run(
                repo,
                &[
                    "config",
                    "--local",
                    "lfs.storage",
                    storage.to_str().context("invalid LFS storage")?,
                ],
            )?;
            self.run(
                repo,
                &[
                    "-c",
                    "lfs.basictransfersonly=true",
                    "lfs",
                    "fetch",
                    "origin",
                    commit,
                ],
            )?;
        }
        ensure!(
            plain_metadata(&object)?.len() == size
                && tree::digest(fs::File::open(&object)?)? == oid,
            "LFS digest/size mismatch"
        );
        fs::copy(object, file)?;
        Ok(Some(oid.into()))
    }
}
