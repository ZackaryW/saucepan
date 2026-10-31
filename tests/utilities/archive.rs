use saucepan::utils::{
    archive::{extract_zip, extract_zip_stripped},
    tree::TreeLimits,
};
use std::{
    fs,
    io::{Cursor, Write},
};
fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn bounded_extraction_and_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    extract_zip(
        Cursor::new(zip(&[("folder/a", b"abc"), ("other", b"x")])),
        &out,
        Some("folder"),
        TreeLimits::default(),
    )
    .unwrap();
    assert_eq!(fs::read(out.join("a")).unwrap(), b"abc");
    assert!(!out.join("other").exists());
    for (i, data) in [
        zip(&[("../escape", b"x")]),
        zip(&[("A", b"x"), ("a", b"y")]),
        zip(&[("A/x", b"x"), ("a/y", b"y")]),
        zip(&[("dir", b"x"), ("dir/file", b"y")]),
        vec![1, 2, 3],
    ]
    .into_iter()
    .enumerate()
    {
        let dest = tmp.path().join(format!("bad{i}"));
        assert!(extract_zip(Cursor::new(data), &dest, None, TreeLimits::default()).is_err());
        assert!(!dest.exists());
    }
    let limit = tmp.path().join("limit");
    assert!(
        extract_zip(
            Cursor::new(zip(&[("selected/a", b"a"), ("outside", b"long")])),
            &limit,
            Some("selected"),
            TreeLimits {
                entries: 10,
                bytes: 2
            }
        )
        .is_err()
    );
    assert!(!limit.exists());
}

#[test]
fn links_corruption_and_occupied_destinations_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink(
            "link",
            "../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let data = writer.finish().unwrap().into_inner();
    assert!(extract_zip(Cursor::new(data), &out, None, TreeLimits::default()).is_err());
    assert!(!out.exists());
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "file",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
    writer.write_all(b"unique payload").unwrap();
    let mut data = writer.finish().unwrap().into_inner();
    let offset = data
        .windows(14)
        .position(|w| w == b"unique payload")
        .unwrap();
    data[offset] ^= 1;
    assert!(extract_zip(Cursor::new(data), &out, None, TreeLimits::default()).is_err());
    assert!(!out.exists());
    fs::create_dir(&out).unwrap();
    fs::write(out.join("sentinel"), b"old").unwrap();
    assert!(
        extract_zip(
            Cursor::new(zip(&[("new", b"new")])),
            &out,
            None,
            TreeLimits::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(out.join("sentinel")).unwrap(), b"old");
}

#[test]
fn stripped_extraction_removes_the_single_root_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let snapshot = zip(&[
        ("owner-repo-0123abc/", b""),
        ("owner-repo-0123abc/README.md", b"readme"),
        ("owner-repo-0123abc/skills/a/SKILL.md", b"skill"),
    ]);
    let all = tmp.path().join("all");
    extract_zip_stripped(
        Cursor::new(snapshot.clone()),
        &all,
        None,
        TreeLimits::default(),
    )
    .unwrap();
    assert_eq!(fs::read(all.join("README.md")).unwrap(), b"readme");
    assert_eq!(fs::read(all.join("skills/a/SKILL.md")).unwrap(), b"skill");
    let skills = tmp.path().join("skills");
    extract_zip_stripped(
        Cursor::new(snapshot),
        &skills,
        Some("skills"),
        TreeLimits::default(),
    )
    .unwrap();
    assert_eq!(fs::read(skills.join("a/SKILL.md")).unwrap(), b"skill");
    assert!(!skills.join("README.md").exists());
}

#[test]
fn stripped_extraction_rejects_other_layouts_and_names_links() {
    let tmp = tempfile::tempdir().unwrap();
    for (i, data) in [
        zip(&[("one/a", b"x"), ("two/b", b"y")]),
        zip(&[("root/a", b"x"), ("loose", b"y")]),
    ]
    .into_iter()
    .enumerate()
    {
        let out = tmp.path().join(format!("layout{i}"));
        let error =
            extract_zip_stripped(Cursor::new(data), &out, None, TreeLimits::default()).unwrap_err();
        assert!(
            error.to_string().contains("one top-level folder"),
            "{error}"
        );
        assert!(!out.exists());
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink(
            "root/docs/link",
            "../x",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let out = tmp.path().join("link");
    let error = extract_zip_stripped(
        Cursor::new(writer.finish().unwrap().into_inner()),
        &out,
        None,
        TreeLimits::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("root/docs/link"), "{error}");
    assert!(!out.exists());
}
