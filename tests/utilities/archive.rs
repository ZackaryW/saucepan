use saucepan::utils::{archive::extract_zip, tree::TreeLimits};
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
