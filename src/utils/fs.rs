use anyhow::Result;
use std::path::Path;

/// Write `content` to `path` atomically via a sibling `.tmp` file and rename.
/// Either the old file or the new file survives a crash — never a partial write.
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_file_contents() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("out.json");
        atomic_write(&path, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
    }

    #[test]
    fn no_tmp_file_left_on_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("out.json");
        atomic_write(&path, b"data").unwrap();
        assert!(!dir.path().join("out.tmp").exists());
    }
}
