use saucepan::{
    core::{Store, retrieval::Retriever, store::Acquirer},
    models::{
        index::Record,
        policy::PayloadVerification,
        retrieval::{CachePolicy, Ownership, RetrieveRequest, SourceInput},
        source::Source,
        store::{AcquisitionRequest, Destination, Format, Selector},
    },
};
use std::{fs, io::Write, path::Path, process::Command};

fn request(source: Source, selector: Selector) -> RetrieveRequest {
    RetrieveRequest {
        source: SourceInput::Inline { source },
        selector,
        asset: Default::default(),
        cache: CachePolicy::Temporary {},
        verification: PayloadVerification::None,
        expected_sha256: None,
    }
}
fn git(path: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn one_repository_supplies_two_independent_offline_subpaths() {
    let root = tempfile::tempdir().unwrap();
    let origin = root.path().join("origin");
    fs::create_dir(&origin).unwrap();
    git(&origin, &["init", "-b", "main"]);
    for folder in ["first", "second"] {
        fs::create_dir(origin.join(folder)).unwrap();
        fs::write(origin.join(folder).join("file"), folder).unwrap();
    }
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "fixture"]);
    let source = Retriever::retrieve(&request(
        Source::Git {
            origin: origin.to_str().unwrap().into(),
        },
        Selector::Ref {
            reference: "main".into(),
        },
    ))
    .unwrap();
    let repository = source.source_path().to_owned();
    fs::rename(&origin, root.path().join("unavailable")).unwrap();
    let first = Acquirer::acquire(
        &source,
        &AcquisitionRequest {
            destination: Destination::Temporary {},
            folder: Some("first".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let second = Acquirer::acquire(
        &source,
        &AcquisitionRequest {
            destination: Destination::Directory {
                path: root.path().join("delivery"),
            },
            folder: Some("second".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(first.path().join("file")).unwrap(), b"first");
    assert_eq!(fs::read(second.path().join("file")).unwrap(), b"second");
    let first_path = first.path().to_owned();
    let second_path = second.path().to_owned();
    drop(source);
    assert!(!repository.exists());
    fs::write(first.path().join("file"), b"changed").unwrap();
    assert_eq!(fs::read(second.path().join("file")).unwrap(), b"second");
    drop(first);
    drop(second);
    assert!(!first_path.exists());
    assert!(second_path.exists());
}

#[test]
fn cached_archive_delivers_caller_and_managed_copies_independent_of_source_purge() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("input.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("folder/file", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"payload").unwrap();
    zip.finish().unwrap();
    let store = Store::create(&root.path().join("root")).unwrap();
    let mut req = request(Source::Local { path: archive }, Selector::Local);
    req.cache = CachePolicy::default();
    let source = store.retrieve(&req).unwrap();
    let Ownership::Managed { uuid } = source.descriptor().ownership else {
        panic!()
    };
    let caller_request = AcquisitionRequest {
        destination: Destination::Directory {
            path: root.path().join("delivery"),
        },
        format: Format::Zip,
        folder: Some("folder".into()),
        ..Default::default()
    };
    let caller = Acquirer::acquire(&source, &caller_request).unwrap();
    assert!(
        store
            .records()
            .unwrap()
            .iter()
            .all(|r| !matches!(r, Record::Output(_)))
    );
    assert!(Acquirer::acquire(&source, &caller_request).is_err());
    let managed = Acquirer::acquire(
        &source,
        &AcquisitionRequest {
            destination: Destination::Managed {},
            format: Format::Zip,
            folder: Some("folder".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!store.purge_retrieval(uuid).unwrap());
    drop(source);
    assert!(store.purge_retrieval(uuid).unwrap());
    assert_eq!(fs::read(caller.path().join("file")).unwrap(), b"payload");
    assert_eq!(fs::read(managed.path().join("file")).unwrap(), b"payload");
    fs::write(caller.path().join("file"), b"changed").unwrap();
    assert_eq!(fs::read(managed.path().join("file")).unwrap(), b"payload");
}

#[test]
fn fresh_handle_cannot_create_managed_effects_or_overwrite_its_input() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("file"), b"original").unwrap();
    let source =
        Retriever::retrieve(&request(Source::Local { path: input }, Selector::Local)).unwrap();
    assert!(Acquirer::acquire(&source, &AcquisitionRequest::default()).is_err());
    assert!(
        Acquirer::acquire(
            &source,
            &AcquisitionRequest {
                destination: Destination::Directory {
                    path: source.source_path().join("nested")
                },
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(!source.source_path().join("nested").exists());
}
