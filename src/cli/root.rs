use crate::error::{Error, ErrorCode};
use std::path::{Path, PathBuf};

pub(super) fn resolve(explicit: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    resolve_with(explicit, std::env::home_dir)
}

fn resolve_with(
    explicit: Option<PathBuf>,
    home: impl FnOnce() -> Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(root) = explicit {
        anyhow::ensure!(
            !root.as_os_str().is_empty(),
            Error::new(ErrorCode::InvalidRequest, "--root must not be empty")
        );
        return Ok(root);
    }
    let home = home().filter(|path| path.is_absolute()).ok_or_else(|| {
        Error::new(
            ErrorCode::InvalidRequest,
            "cannot resolve the user home directory; pass --root PATH",
        )
    })?;
    Ok(home.join(".saucepan"))
}

pub(super) fn require_existing(root: &Path, explicit: bool) -> anyhow::Result<()> {
    if !root.try_exists()? {
        let instruction = if explicit {
            "store root does not exist; run saucepan --root PATH init using the same root"
        } else {
            "default store does not exist; run saucepan init"
        };
        return Err(Error::new(ErrorCode::UnsupportedStore, instruction).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_root_never_needs_home_and_invalid_home_never_uses_cwd() {
        let relative = PathBuf::from("mock");
        assert_eq!(
            resolve_with(Some(relative.clone()), || panic!("home lookup")).unwrap(),
            relative
        );
        for home in [None, Some(PathBuf::new()), Some(PathBuf::from("relative"))] {
            let error = resolve_with(None, || home).unwrap_err();
            assert_eq!(Error::from_anyhow(&error).code, ErrorCode::InvalidRequest);
            assert!(error.to_string().contains("--root"));
        }
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_with(None, || Some(tmp.path().to_owned())).unwrap(),
            tmp.path().join(".saucepan")
        );
    }
}
