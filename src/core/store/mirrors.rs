//! Journal-owned mirror cutover. External destinations never confer ownership.
use super::layout::{Layout, external_path};
use crate::core::{authority::path_contains, materialization, models::*};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MirrorChange {
    pub destination: String,
    pub staged: Option<String>,
    pub backup: String,
    pub previous: Option<MirrorBinding>,
    pub owned: Option<MirrorBinding>,
    pub had_previous: bool,
}

pub(super) fn records(central: &CentralIndex) -> Result<BTreeMap<String, MirrorBinding>> {
    let mut records: BTreeMap<String, MirrorBinding> = BTreeMap::new();
    for mirror in central.units.values().flat_map(|unit| &unit.mirrors).chain(
        central
            .materializations
            .values()
            .flat_map(|binding| &binding.mirrors),
    ) {
        let path = Path::new(&mirror.directory.path);
        if records
            .keys()
            .any(|old| path_contains(Path::new(old), path) || path_contains(path, Path::new(old)))
        {
            return Err(Error::new(
                ErrorKind::Conflict,
                "mirror destinations overlap another binding",
            ));
        }
        records.insert(mirror.directory.path.clone(), mirror.clone());
    }
    Ok(records)
}

impl MirrorChange {
    pub(super) fn new(
        destination: String,
        staged: Option<String>,
        previous: Option<MirrorBinding>,
        owned: Option<MirrorBinding>,
    ) -> Result<Self> {
        let path = external_path(Path::new(&destination))?;
        if previous.is_none() && path.exists() {
            return Err(conflict("mirror destination is not owned by this binding"));
        }
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|_| invalid("OS random source unavailable"))?;
        let backup = path
            .parent()
            .ok_or_else(|| invalid("mirror has no parent"))?
            .join(format!(".saucepan-backup-{}", crate::utils::hex(&nonce)))
            .to_str()
            .ok_or_else(|| invalid("invalid mirror backup path"))?
            .into();
        Ok(Self {
            had_previous: path.exists(),
            destination,
            staged,
            backup,
            previous,
            owned,
        })
    }
    fn validate_paths(&self, layout: &Layout) -> Result<()> {
        let destination = external_path(Path::new(&self.destination))?;
        if destination != Path::new(&self.destination)
            || path_contains(layout.root(), &destination)
            || path_contains(&destination, layout.root())
        {
            return Err(invalid(
                "mirror path overlaps the store or is not canonical",
            ));
        }
        let backup = external_path(Path::new(&self.backup))?;
        let suffix = backup
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(".saucepan-backup-"))
            .ok_or_else(|| invalid("unowned mirror backup"))?;
        if backup.parent() != destination.parent()
            || suffix.len() != 64
            || !suffix.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("unowned mirror backup"));
        }
        if let Some(staged) = &self.staged {
            let staged = external_path(Path::new(staged))?;
            let suffix = staged
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("materialize-"))
                .ok_or_else(|| invalid("unowned mirror staging"))?;
            if staged.parent() != destination.parent()
                || suffix.is_empty()
                || !suffix.bytes().all(|b| b.is_ascii_alphanumeric())
                || staged == destination
                || staged == backup
            {
                return Err(invalid("unowned mirror staging"));
            }
        }
        if self.owned.is_some() != self.staged.is_some()
            || (self.had_previous && self.previous.is_none())
        {
            return Err(invalid("inconsistent mirror publication"));
        }
        for record in self.previous.iter().chain(self.owned.iter()) {
            if record.directory.path != self.destination
                || record.directory.tree_digest != record.artifact.tree_digest
            {
                return Err(invalid("mirror provenance does not match its directory"));
            }
        }
        Ok(())
    }
    pub(super) fn preflight(&self, layout: &Layout) -> Result<()> {
        self.validate_paths(layout)?;
        if Path::new(&self.backup).exists() {
            return Err(conflict("mirror backup destination is occupied"));
        }
        if let Some(previous) = &self.previous {
            materialization::check(
                Path::new(&self.destination),
                &previous.directory.entries,
                !self.had_previous,
            )
            .map_err(|_| conflict("mirror contains modified or unowned entries"))?;
        } else if Path::new(&self.destination).exists() {
            return Err(conflict("mirror destination is not owned by this binding"));
        }
        if let (Some(staged), Some(owned)) = (&self.staged, &self.owned) {
            materialization::check(Path::new(staged), &owned.directory.entries, false)?;
        }
        Ok(())
    }
    pub(super) fn publish(
        &self,
        layout: &Layout,
        mut checkpoint: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        self.preflight(layout)?;
        if self.had_previous {
            std::fs::rename(&self.destination, &self.backup)
                .map_err(|_| invalid("cannot stage previous mirror"))?;
            self.flush_parent()?;
        }
        checkpoint(0)?;
        if let Some(staged) = &self.staged {
            if Path::new(&self.destination).exists() {
                return Err(conflict("mirror destination became occupied"));
            }
            std::fs::rename(staged, &self.destination)
                .map_err(|_| invalid("cannot publish mirror"))?;
            self.flush_parent()?;
        }
        checkpoint(1)
    }
    pub(super) fn validate_recovery(
        &self,
        layout: &Layout,
        selected: bool,
        central: &CentralIndex,
    ) -> Result<()> {
        self.validate_paths(layout)?;
        let records = records(central)?;
        let expected = if selected {
            self.owned.as_ref()
        } else {
            self.previous.as_ref()
        };
        if records.get(&self.destination) != expected {
            return Err(invalid("mirror journal does not match selected binding"));
        }
        let backup_exists = Path::new(&self.backup).exists();
        if backup_exists {
            let previous = self
                .previous
                .as_ref()
                .ok_or_else(|| invalid("unexpected mirror backup"))?;
            materialization::check(
                Path::new(&self.backup),
                &previous.directory.entries,
                selected,
            )?;
        }
        if selected {
            if let Some(owned) = &self.owned {
                materialization::check(
                    Path::new(&self.destination),
                    &owned.directory.entries,
                    false,
                )?;
            } else if Path::new(&self.destination).exists() {
                return Err(invalid("removed mirror destination is occupied"));
            }
        } else if !backup_exists && self.had_previous {
            let previous = self
                .previous
                .as_ref()
                .ok_or_else(|| invalid("missing previous mirror record"))?;
            materialization::check(
                Path::new(&self.destination),
                &previous.directory.entries,
                false,
            )?;
        } else if let Some(owned) = &self.owned {
            materialization::check(Path::new(&self.destination), &owned.directory.entries, true)?;
        } else if Path::new(&self.destination).exists() {
            return Err(invalid("unowned destination obstructs mirror recovery"));
        }
        if let (Some(staged), Some(owned)) = (&self.staged, &self.owned) {
            materialization::check(Path::new(staged), &owned.directory.entries, true)?;
        }
        Ok(())
    }
    pub(super) fn recover(
        &self,
        layout: &Layout,
        selected: bool,
        central: &CentralIndex,
    ) -> Result<()> {
        self.validate_recovery(layout, selected, central)?;
        if selected {
            if let Some(previous) = &self.previous {
                materialization::cleanup(Path::new(&self.backup), &previous.directory.entries)?;
            }
        } else {
            let backup_exists = Path::new(&self.backup).exists();
            if (backup_exists || !self.had_previous)
                && let Some(owned) = &self.owned
            {
                materialization::cleanup(Path::new(&self.destination), &owned.directory.entries)?;
            }
            if backup_exists {
                if Path::new(&self.destination).exists() {
                    return Err(conflict("mirror rollback destination is occupied"));
                }
                std::fs::rename(&self.backup, &self.destination)
                    .map_err(|_| invalid("cannot restore previous mirror"))?;
                self.flush_parent()?;
            }
        }
        if let (Some(staged), Some(owned)) = (&self.staged, &self.owned) {
            materialization::cleanup(Path::new(staged), &owned.directory.entries)?;
        }
        self.flush_parent()
    }
    fn flush_parent(&self) -> Result<()> {
        let parent = Path::new(&self.destination)
            .parent()
            .ok_or_else(|| invalid("mirror has no parent"))?;
        if parent.exists() {
            materialization::sync_dir(parent)?;
        }
        Ok(())
    }
}

pub(super) fn prepare(
    layout: &Layout,
    before: &CentralIndex,
    after: &CentralIndex,
    mut supplied: Vec<MirrorChange>,
) -> Result<Vec<MirrorChange>> {
    let old = records(before)?;
    let new = records(after)?;
    let mut seen = std::collections::BTreeSet::new();
    for change in &supplied {
        if !seen.insert(change.destination.clone())
            || old.get(&change.destination) != change.previous.as_ref()
            || new.get(&change.destination) != change.owned.as_ref()
        {
            return Err(invalid("unbacked mirror publication"));
        }
    }
    for (path, previous) in &old {
        if !new.contains_key(path) && !seen.contains(path) {
            supplied.push(MirrorChange::new(
                path.clone(),
                None,
                Some(previous.clone()),
                None,
            )?);
            seen.insert(path.clone());
        }
    }
    for (path, owned) in &new {
        if old.get(path) != Some(owned) && !seen.contains(path) {
            return Err(invalid("unbacked mirror binding edit"));
        }
    }
    for change in &supplied {
        change.preflight(layout)?;
    }
    Ok(supplied)
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::Integrity, message)
}
fn conflict(message: &str) -> Error {
    Error::new(ErrorKind::Conflict, message)
}
