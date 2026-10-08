use std::{fs::File, io, path::Path};

/// Single-file replacement, not a transaction or directory durability guarantee.
pub fn atomic_write(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    write(tmp.as_file_mut())?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Caller serializes publication and supplies a trusted, existing parent.
pub fn staged_directory(
    path: &Path,
    prepare: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let absent = || match path.symlink_metadata() {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination occupied",
        )),
    };
    absent()?;
    let tmp = tempfile::tempdir_in(
        path.parent()
            .ok_or_else(|| io::Error::other("missing parent"))?,
    )?;
    prepare(tmp.path())?;
    absent()?;
    std::fs::rename(tmp.path(), path)?;
    Ok(())
}
