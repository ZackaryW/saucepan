//! Turn the committed inventory into ordinary files; never create native links.
use crate::utils::{fs::create_directories, links, path::native_relative_path};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

pub(super) fn materialize(
    input: &Path,
    output: &Path,
    entries: &BTreeMap<String, links::Entry>,
    executables: &BTreeSet<String>,
) -> io::Result<BTreeSet<String>> {
    fs::create_dir(output)?;
    let mut resolved_executables = BTreeSet::new();
    links::expand(
        entries,
        links::Limits {
            hops: 64,
            entries: super::super::ZIP_LIMITS.entries,
            bytes: super::super::MAX_BYTES,
        },
        |name, original, entry| {
            let relative = native_relative_path(name)?;
            if matches!(entry, links::Entry::Directory) {
                return create_directories(output, &relative);
            }
            if executables.contains(original) {
                resolved_executables.insert(name.to_owned());
            }
            create_directories(output, relative.parent().unwrap_or(Path::new("")))?;
            let original = input.join(native_relative_path(original)?);
            let destination = output.join(relative);
            let mut source = fs::File::open(&original)?;
            let mut target = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)?;
            io::copy(&mut source, &mut target)?;
            fs::set_permissions(&destination, source.metadata()?.permissions())?;
            Ok(())
        },
    )?;
    Ok(resolved_executables)
}
