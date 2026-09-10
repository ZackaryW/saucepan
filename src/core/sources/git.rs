use super::{Prepared, RemoteUnavailable};
use crate::{
    core::{content, models::Source},
    utils::{fs::staged_directory, hash::sha256, links, path::native_relative_path, tree},
};
mod export;
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
};

fn command(repo: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
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
        .env("GIT_TERMINAL_PROMPT", "0");
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
        cmd.env_remove(name);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
fn output(repo: &Path, args: &[&str]) -> Result<Output> {
    command(repo)
        .args(args)
        .output()
        .context("Git could not be executed")
}
fn run(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = output(repo, args)?;
    ensure!(
        output.status.success(),
        "Git operation failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}
fn text(repo: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(run(repo, args)?)?.trim().to_owned())
}

pub(crate) fn verify_origin(source: &Source, repo: &Path) -> Result<()> {
    let Source::Git { reference, .. } = source else {
        return Ok(());
    };
    ensure!(
        tree::metadata(repo)?.is_dir(),
        "canonical Git repository is missing"
    );
    let configured = text(repo, &["config", "--get-all", "remote.origin.url"])?;
    let effective = text(repo, &["remote", "get-url", "--all", "origin"])?;
    for origin in [configured, effective] {
        ensure!(origin.lines().count() == 1, "ambiguous Git origin");
        ensure!(
            Source::Git {
                origin,
                reference: reference.clone()
            }
            .id()?
                == source.id()?,
            "Git origin differs from encrypted source identity"
        );
    }
    Ok(())
}

fn repository(source: &Source, repo: &Path) -> Result<()> {
    let Source::Git { origin, .. } = source else {
        bail!("expected Git source");
    };
    if !repo.try_exists()? {
        let parent = repo.parent().context("missing repository parent")?;
        staged_directory(repo, |stage| {
            let result = command(parent)
                .args([
                    "clone",
                    "--bare",
                    "--no-hardlinks",
                    "--origin",
                    "origin",
                    "--",
                    origin,
                ])
                .arg(stage)
                .output()?;
            if result.status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other(RemoteUnavailable))
            }
        })?;
    }
    verify_origin(source, repo)
}

pub(crate) fn prepare(source: &Source, work: &Path, pin: Option<&str>) -> Result<Prepared> {
    let Source::Git { reference, .. } = source else {
        bail!("expected Git source");
    };
    let repo = work.join("repo");
    repository(source, &repo)?;
    let revision = resolve(&repo, pin.unwrap_or(reference), pin.is_some())?;
    let directory = tempfile::tempdir_in(work)?;
    let root = directory.path().join("input");
    fs::create_dir(&root)?;
    let mut exported = Exported::default();
    export(
        (source, &repo),
        &revision,
        &root,
        work,
        "",
        &mut exported,
        0,
    )
    .map_err(|error| anyhow::anyhow!("required Git content could not be exported: {error:#}"))?;
    exported.executables = export::materialize(
        &root,
        &directory.path().join("tree"),
        &exported.entries,
        &exported.executables,
    )
    .context("required Git links could not be resolved")?;
    Ok(Prepared {
        directory,
        revision,
        dependencies: exported.dependencies,
        executables: Some(exported.executables),
    })
}

fn resolve(repo: &Path, requested: &str, pinned: bool) -> Result<String> {
    let expression = format!("{requested}^{{commit}}");
    if pinned
        && output(repo, &["cat-file", "-e", &expression])?
            .status
            .success()
    {
        return text(repo, &["rev-parse", "--verify", &expression]);
    }
    if !output(
        repo,
        &["fetch", "--no-tags", "--force", "origin", requested],
    )?
    .status
    .success()
    {
        return Err(RemoteUnavailable.into());
    }
    text(repo, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"])
}

#[derive(Default)]
struct Exported {
    dependencies: BTreeMap<String, String>,
    executables: std::collections::BTreeSet<String>,
    entries: BTreeMap<String, links::Entry>,
    bytes: u64,
}

fn export(
    input: (&Source, &Path),
    revision: &str,
    root: &Path,
    work: &Path,
    prefix: &str,
    exported: &mut Exported,
    depth: usize,
) -> Result<()> {
    let (source, repo) = input;
    ensure!(depth <= 32, "submodule nesting limit exceeded");
    let listing = run(repo, &["ls-tree", "-rtzl", "--full-tree", revision])?;
    for entry in listing.split(|&b| b == 0).filter(|e| !e.is_empty()) {
        let (header, name) = entry.split_at(
            entry
                .iter()
                .position(|&b| b == b'\t')
                .context("invalid Git tree entry")?,
        );
        let path = std::str::from_utf8(&name[1..])?;
        if path
            .split('/')
            .any(|component| component.eq_ignore_ascii_case(".git"))
        {
            continue;
        }
        let relative = native_relative_path(path)?;
        let header = std::str::from_utf8(header)?
            .split_whitespace()
            .collect::<Vec<_>>();
        ensure!(header.len() == 4, "invalid Git tree entry");
        let destination = root.join(&relative);
        let logical = if prefix.is_empty() {
            path.to_owned()
        } else {
            format!("{prefix}/{path}")
        };
        ensure!(
            exported.entries.len() < super::ZIP_LIMITS.entries,
            "Git export entry limit exceeded"
        );
        ensure!(
            !exported.entries.contains_key(&logical),
            "duplicate Git export path"
        );
        if matches!(header[0], "040000" | "160000") {
            exported
                .entries
                .insert(logical.clone(), links::Entry::Directory);
            crate::utils::fs::create_directories(root, &relative)?;
            if header[0] == "040000" {
                continue;
            }
        }
        if header[0] == "160000" {
            let child_origin = submodule_origin(source, repo, revision, path, work)?;
            let child = Source::Git {
                origin: child_origin,
                reference: header[2].into(),
            }
            .canonical()?;
            let cache = work.join("submodules").join(content::digest(&child)?);
            fs::create_dir_all(&cache)?;
            let child_repo = cache.join("repo");
            repository(&child, &child_repo).context("required submodule is unavailable")?;
            let resolved = resolve(&child_repo, header[2], true)
                .context("required submodule commit is unavailable")?;
            ensure!(resolved == header[2], "submodule commit mismatch");
            crate::utils::fs::create_directories(root, &relative)?;
            exported
                .dependencies
                .insert(logical.clone(), resolved.clone());
            export(
                (&child, &child_repo),
                &resolved,
                &destination,
                &cache,
                &logical,
                exported,
                depth + 1,
            )?;
            continue;
        }
        ensure!(
            matches!(header[0], "100644" | "100755" | "120000"),
            "Git special files are unsupported"
        );
        let size: u64 = header[3].parse().context("invalid Git blob size")?;
        exported.bytes = exported
            .bytes
            .checked_add(size)
            .filter(|n| *n <= super::MAX_BYTES)
            .context("Git export byte limit exceeded")?;
        if header[0] == "120000" {
            exported.entries.insert(
                logical,
                links::Entry::Link(run(repo, &["cat-file", "blob", header[2]])?),
            );
            continue;
        }
        if header[0] == "100755" {
            exported.executables.insert(logical.clone());
        }
        crate::utils::fs::create_directories(
            root,
            relative.parent().context("missing export parent")?,
        )?;
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&destination)?;
        let result = command(repo)
            .args(["cat-file", "blob", header[2]])
            .stdout(Stdio::from(file))
            .stderr(Stdio::piped())
            .output()?;
        ensure!(result.status.success(), "required Git blob is unavailable");
        if let Some(oid) = expand_lfs(repo, revision, &destination)? {
            exported.dependencies.insert(format!("lfs:{logical}"), oid);
        }
        let resolved_size = fs::metadata(&destination)?.len();
        exported.bytes = exported
            .bytes
            .checked_sub(size)
            .and_then(|n| n.checked_add(resolved_size))
            .filter(|n| *n <= super::MAX_BYTES)
            .context("Git export byte limit exceeded")?;
        exported
            .entries
            .insert(logical, links::Entry::File(resolved_size));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &destination,
                fs::Permissions::from_mode(if header[0] == "100755" { 0o755 } else { 0o644 }),
            )?;
        }
    }
    Ok(())
}

fn submodule_origin(
    source: &Source,
    repo: &Path,
    revision: &str,
    path: &str,
    work: &Path,
) -> Result<String> {
    let mut config = tempfile::NamedTempFile::new_in(work)?;
    config.write_all(&run(repo, &["show", &format!("{revision}:.gitmodules")])?)?;
    let config_path = config.path().to_str().context("non-UTF-8 temporary path")?;
    let entries = run(
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
    for entry in entries.split(|&b| b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry)?;
        let (key, value) = entry
            .split_once('\n')
            .context("invalid submodule configuration")?;
        if value != path {
            continue;
        }
        let key = format!(
            "{}.url",
            key.strip_suffix(".path").context("invalid submodule key")?
        );
        let locator = text(
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
        if locator.starts_with("./") || locator.starts_with("../") {
            let Source::Git { origin, .. } = source else {
                bail!("expected Git origin");
            };
            return Ok(::url::Url::parse(&format!("{origin}/"))?
                .join(&locator)?
                .to_string());
        }
        return Ok(locator);
    }
    bail!("required submodule URL is missing")
}

fn expand_lfs(repo: &Path, revision: &str, file: &Path) -> Result<Option<String>> {
    let mut head = Vec::new();
    fs::File::open(file)?.take(1025).read_to_end(&mut head)?;
    if !head.starts_with(b"version https://git-lfs.github.com/spec/v1") {
        return Ok(None);
    }
    ensure!(head.len() <= 1024, "malformed LFS pointer");
    let pointer = std::str::from_utf8(&head)?;
    ensure!(
        pointer.lines().count() == 3
            && pointer.lines().next() == Some("version https://git-lfs.github.com/spec/v1"),
        "unsupported or malformed LFS pointer"
    );
    let oid = pointer
        .lines()
        .find_map(|line| line.strip_prefix("oid sha256:"))
        .context("missing LFS SHA-256")?;
    ensure!(
        oid.len() == 64
            && oid
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "invalid LFS SHA-256"
    );
    let size: u64 = pointer
        .lines()
        .find_map(|line| line.strip_prefix("size "))
        .context("missing LFS size")?
        .parse()?;
    ensure!(
        size <= super::MAX_BYTES,
        "LFS object exceeds supported size"
    );
    let storage = repo.join("lfs");
    let object = storage
        .join("objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(oid);
    if !object.try_exists()? {
        // Local config avoids passing a storage override to git-lfs's remote-side
        // subprocess, which would otherwise look for remote objects in our cache.
        run(
            repo,
            &[
                "config",
                "--local",
                "lfs.storage",
                storage.to_str().context("non-UTF-8 LFS storage path")?,
            ],
        )?;
        run(
            repo,
            &[
                "-c",
                "lfs.basictransfersonly=true",
                "lfs",
                "fetch",
                "origin",
                revision,
            ],
        )
        .context("required LFS content could not be resolved")?;
    }
    ensure!(
        tree::metadata(&object)?.len() == size && sha256(fs::File::open(&object)?)? == oid,
        "LFS object digest or size mismatch"
    );
    fs::copy(object, file)?;
    Ok(Some(oid.into()))
}
