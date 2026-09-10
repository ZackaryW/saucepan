//! Resolve a logical tree into ordinary entries without accessing a filesystem.
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io,
};

#[derive(Debug, Clone)]
pub enum Entry {
    Directory,
    File(u64),
    Link(Vec<u8>),
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub hops: usize,
    pub entries: usize,
    pub bytes: u64,
}

/// Emit destination paths and their resolved original entries in parent-first order.
/// The map must include directory parents; its root is implicit. On an error the
/// sink may have received a prefix: callers must stage publication separately.
pub fn expand(
    tree: &BTreeMap<String, Entry>,
    limits: Limits,
    mut emit: impl FnMut(&str, &str, &Entry) -> io::Result<()>,
) -> io::Result<()> {
    if tree.len() > limits.entries {
        return Err(invalid("expanded entry limit exceeded"));
    }
    let mut remaining_entries = limits.entries;
    let mut remaining_bytes = limits.bytes;
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for path in tree.keys() {
        super::path::relative_path(path)?;
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        if !matches!(entry(tree, parent)?, Entry::Directory) {
            return Err(invalid("logical entry parent is not a directory"));
        }
        children.entry(parent).or_default().push(name);
    }
    // Iterative traversal avoids using the native call stack for directory depth.
    let mut work = vec![(String::new(), String::new(), 0, false)];
    let mut ancestors = BTreeSet::new();
    while let Some((output, source, hops, exit)) = work.pop() {
        if exit {
            ancestors.remove(&source);
            continue;
        }
        let (target, hops) = resolve(tree, &source, hops, limits.hops)?;
        let node = entry(tree, &target)?;
        if matches!(node, Entry::Directory) {
            if !ancestors.insert(target.clone()) {
                return Err(invalid("recursive directory alias"));
            }
            work.push((String::new(), target.clone(), hops, true));
            if let Some(names) = children.get(target.as_str()) {
                remaining_entries = remaining_entries
                    .checked_sub(names.len())
                    .ok_or_else(|| invalid("expanded entry limit exceeded"))?;
                for name in names.iter().rev() {
                    work.push((join(&output, name), join(&target, name), hops, false));
                }
            }
        }
        if let Entry::File(size) = node {
            remaining_bytes = remaining_bytes
                .checked_sub(*size)
                .ok_or_else(|| invalid("expanded byte limit exceeded"))?;
        }
        if !output.is_empty() {
            emit(&output, &target, node)?;
        }
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}/{name}")
    }
}
fn entry<'a>(tree: &'a BTreeMap<String, Entry>, path: &str) -> io::Result<&'a Entry> {
    if path.is_empty() {
        Ok(&Entry::Directory)
    } else {
        tree.get(path)
            .ok_or_else(|| invalid(&format!("missing link target: {path}")))
    }
}

fn resolve(
    tree: &BTreeMap<String, Entry>,
    path: &str,
    mut hops: usize,
    limit: usize,
) -> io::Result<(String, usize)> {
    // A true marker ends a link expansion and carries its active link path.
    let mut pending: VecDeque<(String, bool)> =
        path.split('/').map(|p| (p.into(), false)).collect();
    let mut active = BTreeSet::new();
    let mut current: Vec<String> = Vec::new();
    while let Some((component, end_link)) = pending.pop_front() {
        if end_link {
            active.remove(&component);
            continue;
        }
        if !matches!(entry(tree, &current.join("/"))?, Entry::Directory) {
            return Err(invalid("link traverses a file"));
        }
        match component.as_str() {
            "" | "." => continue,
            ".." => {
                current
                    .pop()
                    .ok_or_else(|| invalid("link escapes source root"))?;
                continue;
            }
            _ => current.push(component),
        }
        let candidate = current.join("/");
        if let Entry::Link(bytes) = entry(tree, &candidate)? {
            hops = hops
                .checked_add(1)
                .filter(|n| *n <= limit)
                .ok_or_else(|| invalid("link hop limit exceeded"))?;
            if !active.insert(candidate.clone()) {
                return Err(invalid("link cycle"));
            }
            let target =
                std::str::from_utf8(bytes).map_err(|_| invalid("link target is not UTF-8"))?;
            if target.is_empty()
                || target.starts_with('/')
                || target.contains(['\\', '\0'])
                || (target.as_bytes().get(1) == Some(&b':')
                    && target.as_bytes()[0].is_ascii_alphabetic())
            {
                return Err(invalid("link target must be a relative POSIX path"));
            }
            current.pop();
            pending.push_front((candidate, true));
            for part in target.split('/').rev() {
                pending.push_front((part.into(), false));
            }
        }
    }
    Ok((current.join("/"), hops))
}

#[cfg(test)]
mod tests;
