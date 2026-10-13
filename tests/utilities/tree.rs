use saucepan::utils::tree::{TreeLimits, copy_tree, inspect};
use std::fs;
#[test]
fn unhashed_copy_preserves_structure_limits_and_independence() {
    use saucepan::utils::tree::copy_tree_unhashed;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("nested/file"), b"original").unwrap();
    let out = root.path().join("out");
    copy_tree_unhashed(&source, &out, TreeLimits::default()).unwrap();
    assert_eq!(
        inspect(&source, TreeLimits::default()).unwrap(),
        inspect(&out, TreeLimits::default()).unwrap()
    );
    fs::write(source.join("nested/file"), b"changed").unwrap();
    assert_eq!(fs::read(out.join("nested/file")).unwrap(), b"original");
    assert!(
        copy_tree_unhashed(
            &source,
            &root.path().join("limited"),
            TreeLimits {
                entries: 8,
                bytes: 2
            }
        )
        .is_err()
    );
}
#[test]
fn tree_copy_is_bounded_and_independent() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("in");
    fs::create_dir_all(input.join("nested/empty")).unwrap();
    fs::write(input.join("nested/data"), [0, 255, 42]).unwrap();
    let out = temp.path().join("out");
    let manifest = copy_tree(&input, &out, TreeLimits::default()).unwrap();
    assert_eq!(manifest, inspect(&out, TreeLimits::default()).unwrap());
    fs::write(input.join("nested/data"), "edited").unwrap();
    assert_eq!(fs::read(out.join("nested/data")).unwrap(), [0, 255, 42]);
    assert!(
        copy_tree(
            &input,
            &temp.path().join("bounded"),
            TreeLimits {
                entries: 10,
                bytes: 2
            }
        )
        .is_err()
    );
    assert!(
        copy_tree(
            &input,
            &temp.path().join("count"),
            TreeLimits {
                entries: 1,
                bytes: 100
            }
        )
        .is_err()
    );
    assert!(
        copy_tree(
            &temp.path().join("missing"),
            &temp.path().join("missing-out"),
            TreeLimits::default()
        )
        .is_err()
    );
}
#[test]
fn rejects_filesystem_links() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::create_dir(&input).unwrap();
    let outside = tmp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(input.join("link"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(status.status.success());
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, input.join("link")).unwrap();
    assert!(copy_tree(&input, &tmp.path().join("out"), TreeLimits::default()).is_err());
}
