//! Composition tests for the helpers; these do not implement an application index.
use super::{archive, crypto, fs, hash, json, lock::FileLock, tree};
use std::{
    fs::File,
    io::{Cursor, Read, Write},
    time::Duration,
};

#[test]
fn locked_export_authenticated_storage_extraction_and_independent_copy_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let source = root.join("input");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("nested/file"), b"original bytes").unwrap();
    let _lock = FileLock::acquire(root.join("lock"), Duration::ZERO).unwrap();
    let archive = root.join("content.zip");
    fs::atomic_write(&archive, |file| {
        archive::write_zip(&source, file, |_, _| true)?;
        Ok(())
    })
    .unwrap();
    let digest = hash::sha256(File::open(&archive).unwrap()).unwrap();
    let metadata =
        json::sorted_json(&serde_json::json!({"hash": digest, "paths": ["nested/file"]})).unwrap();
    let encryption_key = crypto::derive_key::<32>(&[7; 32], None, b"example encryption").unwrap();
    let authentication_key =
        crypto::derive_key::<32>(&[7; 32], None, b"example authentication").unwrap();
    let tag = crypto::authenticate(authentication_key.as_ref(), metadata.as_slice()).unwrap();
    let sealed = crypto::seal(&encryption_key, &metadata, b"example context").unwrap();
    let encrypted = root.join("encrypted");
    fs::atomic_write(&encrypted, |file| {
        file.write_all(&sealed.nonce)?;
        file.write_all(&sealed.ciphertext)
    })
    .unwrap();
    let mut reader = File::open(encrypted).unwrap();
    let mut nonce = [0; 24];
    reader.read_exact(&mut nonce).unwrap();
    let mut ciphertext = Vec::new();
    reader.read_to_end(&mut ciphertext).unwrap();
    let restored = crypto::open(
        &encryption_key,
        &crypto::Sealed { nonce, ciphertext },
        b"example context",
    )
    .unwrap();
    crypto::verify(authentication_key.as_ref(), restored.as_slice(), &tag).unwrap();
    assert_eq!(&*restored, &metadata);
    let output = root.join("output");
    archive::extract_zip(
        File::open(&archive).unwrap(),
        &output,
        Some("nested"),
        archive::Limits {
            entries: 10,
            bytes: 1000,
        },
    )
    .unwrap();
    let copy = root.join("copy");
    tree::copy_tree(&output, &copy, |_, _| true).unwrap();
    std::fs::write(copy.join("file"), b"edited").unwrap();
    assert_eq!(
        std::fs::read(output.join("file")).unwrap(),
        b"original bytes"
    );
    assert_eq!(hash::sha256(File::open(archive).unwrap()).unwrap(), digest);
}

#[test]
fn failed_zip_export_does_not_replace_previously_published_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("output.zip");
    std::fs::write(&output, b"previous").unwrap();
    assert!(
        fs::atomic_write(&output, |file| {
            archive::write_zip(dir.path().join("missing"), file, |_, _| true)?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(std::fs::read(output).unwrap(), b"previous");
    assert!(
        archive::extract_zip(
            Cursor::new(b"invalid"),
            dir.path().join("unpublished"),
            None,
            archive::Limits {
                entries: 10,
                bytes: 1000
            }
        )
        .is_err()
    );
    assert!(!dir.path().join("unpublished").exists());
}
