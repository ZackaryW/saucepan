use super::identify;
use crate::core::models::*;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_GIT_OUTPUT: u64 = 64 * 1024 * 1024;
mod lfs;
mod submodules;

pub(in crate::core) struct GitRepository {
    path: PathBuf,
    expected: SourceIdentity,
}
pub(in crate::core) struct GitPin {
    repository: GitRepository,
    reference: String,
    commit: String,
}
impl Drop for GitPin {
    fn drop(&mut self) {
        if self.repository.verify_origin().is_ok() {
            let _ = self
                .repository
                .command(&["update-ref", "-d", &self.reference, &self.commit]);
        }
    }
}

impl GitRepository {
    pub(in crate::core) fn temporary_pin(&self, commit: &str) -> Result<GitPin> {
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|_| source("cannot create temporary Git reference"))?;
        let reference = format!("refs/saucepan/preparing/{}", crate::utils::hex(&nonce));
        self.pin(&reference, commit)?;
        Ok(GitPin {
            repository: Self {
                path: self.path.clone(),
                expected: self.expected.clone(),
            },
            reference,
            commit: commit.into(),
        })
    }
    /// The caller supplies an absent path inside its owned staging directory.
    pub(in crate::core) fn clone_into(path: &Path, expected: &SourceIdentity) -> Result<Self> {
        if path.exists() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "repository destination already exists",
            ));
        }
        let transport = transport(expected);
        // Check native URL rewrites before any network request, using an empty
        // local repository so remote get-url applies Git's own rewrite rules.
        let probe = tempfile::tempdir_in(
            path.parent()
                .ok_or_else(|| source("invalid clone staging"))?,
        )
        .map_err(|_| source("cannot prepare origin verification"))?;
        let probe_repo = Self {
            path: probe.path().join("probe.git"),
            expected: expected.clone(),
        };
        let mut command = base_command();
        command
            .args(["init", "--bare", "--template="])
            .arg(git_path(&probe_repo.path)?);
        run(command, probe.path(), "cannot initialize origin probe")?;
        probe_repo.command(&["remote", "add", "origin", &transport])?;
        probe_repo.verify_origin()?;
        let mut command = base_command();
        command
            .args([
                "clone",
                "--bare",
                "--no-local",
                "--no-hardlinks",
                "--template=",
                "--",
                &transport,
            ])
            .arg(git_path(path)?);
        run(command, probe.path(), "Git clone failed")?;
        let repository = Self {
            path: path.into(),
            expected: expected.clone(),
        };
        repository.verify_origin()?;
        Ok(repository)
    }
    pub(in crate::core) fn open(path: &Path, expected: &SourceIdentity) -> Result<Self> {
        let repository = Self {
            path: path.into(),
            expected: expected.clone(),
        };
        if repository.text(&["rev-parse", "--is-bare-repository"])? != "true" {
            return Err(integrity("managed repository is not bare"));
        }
        repository.verify_origin()?;
        Ok(repository)
    }
    /// Required before every fetch AND every committed-content/cache read.
    pub(in crate::core) fn verify_origin(&self) -> Result<()> {
        for args in [
            vec!["config", "--get-all", "remote.origin.url"],
            vec!["remote", "get-url", "--all", "origin"],
        ] {
            let bytes = self
                .command(&args)
                .map_err(|_| integrity("Git fetch origin is missing or invalid"))?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| integrity("Git fetch origin is not UTF-8"))?;
            let origins: Vec<_> = text.lines().collect();
            if origins.len() != 1 || origins[0].is_empty() {
                return Err(integrity("Git fetch origin is ambiguous"));
            }
            let identity = identify(
                &SourceLocator {
                    backend: Backend::Git,
                    origin: origins[0].into(),
                },
                &self.path,
            )
            .map_err(|_| integrity("Git fetch origin cannot be verified"))?;
            if identity.id != self.expected.id || identity.origin != self.expected.origin {
                return Err(integrity(
                    "Git fetch origin does not match the encrypted source identity",
                ));
            }
        }
        Ok(())
    }
    pub(in crate::core) fn resolve(&self, revision: &Revision) -> Result<String> {
        self.verify_origin()?;
        let target = match revision {
            Revision::DefaultBranch => "HEAD".to_owned(),
            Revision::Branch(name) => format!("refs/heads/{name}"),
            Revision::Tag(name) => format!("refs/tags/{name}"),
            Revision::Commit(commit) => {
                let commit = commit.to_ascii_lowercase();
                if let Ok(found) = self.commit(&commit) {
                    return Ok(found);
                }
                commit
            }
        };
        // Explicit refspecs avoid consulting potentially stale clone refs or a
        // modified configured fetch mapping. Never reuse current after failure.
        self.command(&[
            "fetch",
            "--no-tags",
            "--no-recurse-submodules",
            "--no-write-fetch-head",
            "origin",
            &format!("+{target}:refs/saucepan/resolved"),
        ])?;
        self.verify_origin()?;
        self.commit("refs/saucepan/resolved")
    }
    pub(in crate::core) fn pin(&self, reference: &str, commit: &str) -> Result<()> {
        if !reference.starts_with("refs/saucepan/")
            || reference.contains("..")
            || reference.chars().any(char::is_whitespace)
        {
            return Err(integrity("invalid managed Git reference"));
        }
        self.command(&["update-ref", reference, commit])?;
        Ok(())
    }
    /// Reconcile only Saucepan's artifact/dependency pins, preserving all other
    /// refs. The coordinator supplies the complete authenticated reference set.
    pub(in crate::core) fn reconcile_pins(
        &self,
        expected: &std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        self.verify_origin()?;
        let text = self.text(&[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/saucepan/artifacts/",
            "refs/saucepan/dependencies/",
        ])?;
        let mut actual = std::collections::BTreeMap::new();
        for line in text.lines() {
            let (reference, commit) = line
                .split_once(' ')
                .ok_or_else(|| integrity("invalid managed Git reference listing"))?;
            validate_pin(reference, commit)?;
            actual.insert(reference.to_owned(), commit.to_owned());
        }
        // Restore missing pins from authenticated inputs before retiring any.
        for (reference, commit) in expected {
            validate_pin(reference, commit)?;
            if let Some(actual) = actual.get(reference) {
                if actual != commit {
                    return Err(integrity("managed artifact pin was modified"));
                }
            } else {
                self.pin(reference, commit)?;
            }
        }
        for (reference, commit) in actual {
            if !expected.contains_key(&reference) {
                self.command(&["update-ref", "-d", &reference, &commit])?;
            }
        }
        self.verify_origin()
    }
    fn commit(&self, reference: &str) -> Result<String> {
        let commit = self.text(&["rev-parse", "--verify", &format!("{reference}^{{commit}}")])?;
        if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(integrity("Git returned an invalid commit identity"));
        }
        Ok(commit)
    }
    pub(in crate::core) fn tree(&self, commit: &str, selection: &str) -> Result<Vec<u8>> {
        self.verify_origin()?;
        let tree = if selection == "." {
            format!("{commit}^{{tree}}")
        } else {
            format!("{commit}:{selection}")
        };
        if self.text(&["cat-file", "-t", &tree])? != "tree" {
            return Err(source("selection is not a committed directory"));
        }
        self.command(&["ls-tree", "-r", "-z", &tree])
    }
    pub(in crate::core) fn archive(
        &self,
        commit: &str,
        selection: &str,
        staging: &Path,
    ) -> Result<Vec<u8>> {
        self.verify_origin()?;
        // An empty Git view shares committed objects only. Its empty info/ and
        // disabled global attributes prevent local attribute overrides from
        // changing an export. Archive the original commit to retain ancestors.
        let view =
            tempfile::tempdir_in(staging).map_err(|_| source("cannot prepare committed export"))?;
        let view_path = view.path().join("view.git");
        let mut init = base_command();
        init.args(["init", "--bare", "--template="])
            .arg(git_path(&view_path)?);
        run(init, staging, "cannot prepare committed export")?;
        let mut command = base_command();
        command.arg("--git-dir").arg(git_path(&view_path)?).args([
            "-c",
            "core.attributesFile=",
            "archive",
            "--format=zip",
            commit,
        ]);
        if selection != "." {
            command.arg("--").arg(format!("{selection}/"));
        }
        command
            .env(
                "GIT_OBJECT_DIRECTORY",
                git_path(&self.path.join("objects"))?,
            )
            .env("GIT_ATTR_NOSYSTEM", "1");
        run(command, staging, "Git committed-tree export failed")
    }
    fn text(&self, args: &[&str]) -> Result<String> {
        let bytes = self.command(args)?;
        let text =
            String::from_utf8(bytes).map_err(|_| source("Git returned non-UTF-8 metadata"))?;
        Ok(text.trim_end_matches(['\r', '\n']).into())
    }
    fn command(&self, args: &[&str]) -> Result<Vec<u8>> {
        let mut command = base_command();
        command
            .arg("--git-dir")
            .arg(git_path(&self.path)?)
            .args(args);
        run(command, &self.path, "Git source operation failed")
    }
}
fn validate_pin(reference: &str, commit: &str) -> Result<()> {
    let id = reference
        .strip_prefix("refs/saucepan/artifacts/")
        .or_else(|| reference.strip_prefix("refs/saucepan/dependencies/"))
        .ok_or_else(|| integrity("invalid managed Git pin"))?;
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !matches!(commit.len(), 40 | 64)
        || !commit.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(integrity("invalid managed Git pin"));
    }
    Ok(())
}
fn transport(source: &SourceIdentity) -> String {
    if !source.locator.contains(':')
        && !Path::new(&source.locator).is_absolute()
        && source.origin.starts_with("github:")
    {
        format!("https://github.com/{}.git", &source.origin[7..])
    } else if source.origin.starts_with("file:") {
        source.origin.clone()
    } else {
        source.locator.clone()
    }
}
fn base_command() -> Command {
    let mut command = Command::new("git");
    command.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "maintenance.auto=false",
        "-c",
        "gc.auto=0",
        "-c",
        "http.followRedirects=false",
        "-c",
        "core.longpaths=true",
    ]);
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
    ] {
        command.env_remove(key);
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    command
}
fn run(mut command: Command, capture_root: &Path, failure: &str) -> Result<Vec<u8>> {
    run_limited(&mut command, capture_root, failure, MAX_GIT_OUTPUT)
}
fn run_limited(
    command: &mut Command,
    capture_root: &Path,
    failure: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    let mut output =
        tempfile::tempfile_in(capture_root).map_err(|_| source("cannot capture Git result"))?;
    command.stdout(Stdio::from(
        output
            .try_clone()
            .map_err(|_| source("cannot capture Git result"))?,
    ));
    let mut child = command
        .spawn()
        .map_err(|_| source("Git executable is unavailable"))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(source(failure));
                }
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline || !file_size(&output).is_ok_and(|size| size <= limit)
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(source("Git operation exceeded its time or output limit"));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(source(failure));
            }
        }
    }
    if file_size(&output)? > limit {
        return Err(source("Git result exceeds size limit"));
    }
    output
        .seek(SeekFrom::Start(0))
        .map_err(|_| source("cannot read Git result"))?;
    let mut bytes = Vec::new();
    output
        .read_to_end(&mut bytes)
        .map_err(|_| source("cannot read Git result"))?;
    Ok(bytes)
}
// Git for Windows accepts normal drive/UNC paths, not Rust's verbatim prefix.
// Keep the canonical path for filesystem checks and convert only at this edge.
fn git_path(path: &Path) -> Result<String> {
    let path = path
        .to_str()
        .ok_or_else(|| source("Git path is not UTF-8"))?;
    #[cfg(windows)]
    {
        let normalized = if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
            format!("//{rest}")
        } else {
            path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
        };
        Ok(normalized.replace('\\', "/"))
    }
    #[cfg(not(windows))]
    Ok(path.into())
}
fn file_size(file: &File) -> Result<u64> {
    file.metadata()
        .map(|m| m.len())
        .map_err(|_| source("cannot inspect Git result"))
}
fn source(message: &str) -> Error {
    Error::new(ErrorKind::Source, message)
}
fn integrity(message: &str) -> Error {
    Error::new(ErrorKind::Integrity, message)
}

#[cfg(test)]
pub(in crate::core) mod tests {
    use super::*;
    pub(in crate::core) fn git(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    pub(in crate::core) fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-b", "main"]);
        git(dir.path(), &["config", "user.email", "test@example.test"]);
        git(dir.path(), &["config", "user.name", "Fixture"]);
        std::fs::write(dir.path().join("file.txt"), "one").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "one"]);
        dir
    }
    #[test]
    fn tracks_remote_but_commit_pin_remains_fixed_and_origin_repointing_fails() {
        let remote = fixture();
        let other = fixture();
        let stage = tempfile::tempdir().unwrap();
        let identity = identify(
            &Recipe::git(remote.path().to_string_lossy()).source,
            remote.path(),
        )
        .unwrap();
        let repo = GitRepository::clone_into(&stage.path().join("repo.git"), &identity).unwrap();
        let first = repo.resolve(&Revision::DefaultBranch).unwrap();
        assert_eq!(first, git(remote.path(), &["rev-parse", "HEAD"]));
        std::fs::write(remote.path().join("file.txt"), "two").unwrap();
        git(remote.path(), &["commit", "-am", "two"]);
        let second = repo.resolve(&Revision::Branch("main".into())).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            repo.resolve(&Revision::Commit(first.clone())).unwrap(),
            first
        );
        repo.pin("refs/saucepan/current/test", &first).unwrap();
        git(&repo.path, &["gc", "--prune=now"]);
        assert_eq!(
            repo.resolve(&Revision::Commit(first.clone())).unwrap(),
            first
        );
        git(
            &repo.path,
            &[
                "remote",
                "set-url",
                "origin",
                other.path().to_str().unwrap(),
            ],
        );
        assert_eq!(repo.verify_origin().unwrap_err().kind, ErrorKind::Integrity);
        assert!(repo.resolve(&Revision::Commit(first)).is_err());
    }
    #[test]
    fn effective_rewrite_and_multiple_origins_are_rejected() {
        let remote = fixture();
        let other = fixture();
        let stage = tempfile::tempdir().unwrap();
        let identity = identify(
            &Recipe::git(remote.path().to_string_lossy()).source,
            remote.path(),
        )
        .unwrap();
        let repo = GitRepository::clone_into(&stage.path().join("repo.git"), &identity).unwrap();
        let target = identify(
            &Recipe::git(other.path().to_string_lossy()).source,
            other.path(),
        )
        .unwrap();
        let key = format!("url.{}.insteadOf", target.origin);
        git(&repo.path, &["config", &key, &identity.origin]);
        assert_eq!(repo.verify_origin().unwrap_err().kind, ErrorKind::Integrity);
        git(&repo.path, &["config", "--unset-all", &key]);
        git(
            &repo.path,
            &["config", "--add", "remote.origin.url", &target.origin],
        );
        assert_eq!(repo.verify_origin().unwrap_err().kind, ErrorKind::Integrity);
    }
}
