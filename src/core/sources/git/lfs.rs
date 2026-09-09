//! Native LFS transport in an isolated committed-object view. No checkout or
//! repository-defined clean/smudge/extension command participates in export.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;

pub(in crate::core) struct LfsDownload {
    view: tempfile::TempDir,
    objects: PathBuf,
}

impl GitRepository {
    pub(in crate::core) fn lfs_download(
        &self,
        commit: &str,
        staging: &Path,
    ) -> Result<LfsDownload> {
        self.verify_origin()?;
        let view = tempfile::tempdir_in(staging)
            .map_err(|_| source("cannot prepare Git LFS transport"))?;
        let path = view.path().join("view.git");
        let mut init = base_command();
        init.args(["init", "--bare", "--template="])
            .arg(git_path(&path)?);
        run(init, view.path(), "cannot prepare Git LFS transport")?;
        let transport = self.text(&["remote", "get-url", "origin"])?;
        let mut remote = base_command();
        remote
            .arg("--git-dir")
            .arg(git_path(&path)?)
            .args(["remote", "add", "origin", &transport]);
        run(remote, view.path(), "cannot prepare Git LFS origin")?;
        let download = LfsDownload {
            view,
            objects: self.path.join("objects"),
        };
        // Ask native LFS to derive its default endpoint from the already
        // verified Git origin. No network is used by `lfs env`.
        let empty_config = download.view.path().join("empty-config");
        std::fs::write(&empty_config, b"")
            .map_err(|_| source("cannot prepare LFS configuration"))?;
        let mut defaults = download.command()?;
        defaults
            .env("GIT_CONFIG_GLOBAL", git_path(&empty_config)?)
            .env("GIT_CONFIG_SYSTEM", git_path(&empty_config)?)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .env("GIT_CONFIG_COUNT", "0")
            .args(["lfs", "env"]);
        let expected = endpoint(&run(
            defaults,
            download.view.path(),
            "Git LFS is unavailable",
        )?)
        .map_err(|_| integrity("missing default Git LFS endpoint"))?;
        // The committed .lfsconfig is visible only after default discovery.
        // HEAD points at immutable input; no working tree or local repo config
        // is copied into this view.
        std::fs::write(path.join("HEAD"), format!("{commit}\n"))
            .map_err(|_| source("cannot select Git LFS input"))?;
        let mut actual = download.command()?;
        actual.args(["lfs", "env"]);
        let mut configured = download.command()?;
        configured
            .arg("-c")
            .arg(format!(
                "include.path={}",
                git_path(&self.path.join("config"))?
            ))
            .args(["lfs", "env"]);
        if endpoint(&run(
            actual,
            download.view.path(),
            "cannot verify Git LFS endpoint",
        )?)
        .map_err(|_| integrity("missing committed Git LFS endpoint"))?
            != expected
            || endpoint(&run(
                configured,
                download.view.path(),
                "cannot verify configured Git LFS endpoint",
            )?)
            .map_err(|_| integrity("missing configured Git LFS endpoint"))?
                != expected
        {
            return Err(Error::new(
                ErrorKind::Authority,
                "Git LFS endpoint is outside the enrolled source; custom endpoints require explicit source support",
            ));
        }
        // Do not run user-configured extension/transfer programs as a way to
        // complete a recipe. Native file, HTTP and SSH adapters remain usable.
        let mut config = download.command()?;
        config.args(["config", "--null", "--list"]);
        let bytes = run(
            config,
            download.view.path(),
            "cannot inspect Git LFS configuration",
        )?;
        for entry in bytes.split(|b| *b == 0) {
            let key = entry.split(|b| *b == b'\n').next().unwrap_or_default();
            let key = String::from_utf8_lossy(key).to_ascii_lowercase();
            if key.starts_with("lfs.customtransfer.")
                || key.starts_with("lfs.extension.")
                || (key.starts_with("lfs.") && key.ends_with("standalonetransferagent"))
            {
                return Err(Error::new(
                    ErrorKind::Compatibility,
                    "custom Git LFS transfer and extension commands are not supported by export v1",
                ));
            }
        }
        self.verify_origin()?;
        Ok(download)
    }
}

impl LfsDownload {
    fn command(&self) -> Result<Command> {
        let mut command = base_command();
        command
            .current_dir(self.view.path())
            .arg("--git-dir")
            .arg(git_path(&self.view.path().join("view.git"))?)
            .env("GIT_OBJECT_DIRECTORY", git_path(&self.objects)?)
            .env_remove("GIT_LFS_PROGRESS")
            .env("GIT_LFS_SKIP_SMUDGE", "0")
            .env("GIT_LFS_SKIP_DOWNLOAD_ERRORS", "0")
            .args([
                "-c",
                "lfs.fetchinclude=",
                "-c",
                "lfs.fetchexclude=",
                "-c",
                "lfs.skipdownloaderrors=false",
                "-c",
                "lfs.transfer.maxretries=1",
                "-c",
                "lfs.transfer.enablehrefrewrite=false",
                "-c",
                "lfs.concurrenttransfers=1",
            ]);
        Ok(command)
    }
    pub(in crate::core) fn object(&self, object: &LfsObject) -> Result<Vec<u8>> {
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {}\n",
            object.oid, object.size
        );
        let mut input = tempfile::tempfile_in(self.view.path())
            .map_err(|_| source("cannot prepare LFS pointer"))?;
        input
            .write_all(pointer.as_bytes())
            .and_then(|_| input.seek(SeekFrom::Start(0)))
            .map_err(|_| source("cannot prepare LFS pointer"))?;
        let mut command = self.command()?;
        command
            .args(["lfs", "smudge", "--", &object.path])
            .stdin(Stdio::from(input));
        let bytes = run_limited(
            &mut command,
            self.view.path(),
            "required Git LFS object could not be resolved",
            object.size.saturating_add(1),
        )?;
        // Always validate the pointer's declaration, even with optional repeat
        // verification off. Never publish a smudge fallback pointer as content.
        if bytes.len() as u64 != object.size
            || crate::utils::hex(&Sha256::digest(&bytes)) != object.oid
        {
            return Err(integrity(
                "Git LFS object does not match its declared SHA-256 and size",
            ));
        }
        Ok(bytes)
    }
}

fn endpoint(bytes: &[u8]) -> Result<String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| integrity("invalid Git LFS endpoint metadata"))?;
    let mut endpoints = text
        .lines()
        .filter_map(|line| line.strip_prefix("Endpoint="));
    let value = endpoints
        .next()
        .and_then(|value| value.rsplit_once(" (auth=").map(|(url, _)| url))
        .filter(|url| !url.is_empty())
        .ok_or_else(|| integrity("missing Git LFS source endpoint"))?;
    if endpoints.next().is_some() {
        return Err(integrity("ambiguous Git LFS source endpoint"));
    }
    Ok(value.to_owned())
}
