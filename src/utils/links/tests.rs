use super::*;

fn link(target: &str) -> Entry {
    Entry::Link(target.as_bytes().to_vec())
}
fn base() -> BTreeMap<String, Entry> {
    [
        ("a", Entry::Directory),
        ("b", Entry::Directory),
        ("b/deep", Entry::Directory),
        ("b/file", Entry::File(3)),
        ("a/file", Entry::File(2)),
    ]
    .into_iter()
    .map(|(p, e)| (p.into(), e))
    .collect()
}
fn collect(tree: &BTreeMap<String, Entry>) -> io::Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    expand(
        tree,
        Limits {
            hops: 64,
            entries: 1000,
            bytes: 1000,
        },
        |path, target, _| {
            result.insert(path.into(), target.into());
            Ok(())
        },
    )?;
    Ok(result)
}

#[test]
fn files_directories_chains_and_shared_targets() {
    let mut tree = base();
    tree.insert("a/alias".into(), link("../b/file"));
    tree.insert("chain".into(), link("a/alias"));
    tree.insert("copy".into(), link("./b"));
    tree.insert("copy2".into(), link("b"));
    let result = collect(&tree).unwrap();
    for alias in ["a/alias", "chain", "copy/file", "copy2/file"] {
        assert_eq!(result[alias], "b/file");
    }
    assert_eq!(result["copy/deep"], "b/deep");
}

#[test]
fn intermediate_alias_resolves_before_parent_component() {
    let mut tree = base();
    tree.insert("a/to-deep".into(), link("../b/deep"));
    tree.insert("pick".into(), link("a/to-deep/../file"));
    assert_eq!(collect(&tree).unwrap()["pick"], "b/file");
}

#[test]
fn malformed_external_missing_and_file_traversal_fail() {
    for bytes in [
        b"".as_slice(),
        b"/etc/passwd",
        b"C:/host",
        b"C:host",
        b"//host/share",
        b"..\\host",
        b"../a/file",
        b"missing",
        b"b/file/..",
        b"b/file/.",
        b".git/../b/file",
        b"bad\0path",
        &[0xff],
    ] {
        let mut tree = base();
        tree.insert("alias".into(), Entry::Link(bytes.to_vec()));
        assert!(collect(&tree).is_err(), "accepted {bytes:?}");
    }
}

#[test]
fn link_and_directory_cycles_fail() {
    for target in ["alias", ".", "a"] {
        let mut tree = base();
        tree.insert("alias".into(), link(target));
        tree.insert("a/back".into(), link("../alias"));
        assert!(collect(&tree).is_err());
    }
    let mut tree = base();
    tree.insert("one".into(), link("two"));
    tree.insert("two".into(), link("one"));
    assert!(collect(&tree).is_err());
}

#[test]
fn expansion_counts_alias_copies_and_hops() {
    let tree = [
        ("f".into(), Entry::File(3)),
        ("a".into(), link("f")),
        ("b".into(), link("a")),
    ]
    .into();
    let run = |hops, entries, bytes| {
        expand(
            &tree,
            Limits {
                hops,
                entries,
                bytes,
            },
            |_, _, _| Ok(()),
        )
    };
    assert!(run(2, 3, 9).is_ok());
    assert!(run(1, 3, 9).is_err());
    assert!(run(2, 2, 9).is_err());
    assert!(run(2, 3, 8).is_err());
    let huge = [
        ("huge".into(), Entry::File(u64::MAX)),
        ("alias".into(), link("huge")),
    ]
    .into();
    assert!(
        expand(
            &huge,
            Limits {
                hops: 64,
                entries: 3,
                bytes: u64::MAX
            },
            |_, _, _| Ok(())
        )
        .is_err()
    );
}

#[test]
fn directory_fanout_is_bounded_and_callback_errors_propagate() {
    let mut tree = base();
    tree.insert("copy".into(), link("b"));
    let mut emitted = 0;
    assert!(
        expand(
            &tree,
            Limits {
                hops: 64,
                entries: tree.len(),
                bytes: 1000
            },
            |_, _, _| {
                emitted += 1;
                Ok(())
            }
        )
        .is_err()
    );
    assert!(emitted <= tree.len());
    let error = expand(
        &base(),
        Limits {
            hops: 64,
            entries: 100,
            bytes: 1000,
        },
        |_, _, _| Err(io::Error::other("sink failed")),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "sink failed");
}
