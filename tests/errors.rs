mod native_root;
use saucepan::{
    error::{Error, ErrorCode},
    models::{
        policy::{Eviction, Identical},
        source::Source,
        store::{Request, Selector, VerificationMode},
    },
};
use std::fs;
use uuid::Uuid;

fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}

#[test]
fn native_store_failures_have_codes_without_message_matching() {
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    assert_eq!(
        code(store.application(Uuid::new_v4()).unwrap_err()),
        ErrorCode::UnknownApplication
    );
    assert_eq!(
        code(store.output(Uuid::new_v4()).unwrap_err()),
        ErrorCode::UnknownOutput
    );
    assert_eq!(
        code(store.register_app(" ".into()).unwrap_err()),
        ErrorCode::InvalidRequest
    );
    let app = store.register_app("consumer".into()).unwrap();
    assert_eq!(
        code(
            store
                .lease_claim(app.uuid, Uuid::new_v4(), VerificationMode::Metadata)
                .err()
                .unwrap()
        ),
        ErrorCode::UnknownClaim
    );
    let path = tmp.path().join("input");
    fs::write(&path, b"first").unwrap();
    let source = store
        .register_source(Source::Local { path: path.clone() })
        .unwrap();
    let mut request = Request {
        adopted: None,
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.policies.eviction = Eviction::Latest { keep: 0 };
    assert_eq!(
        code(store.acquire(&request).unwrap_err()),
        ErrorCode::InvalidRequest
    );
    request.policies.eviction = Eviction::Latest { keep: 1 };
    request.policies.identical = Identical::NewEntry;
    let old = store.acquire(&request).unwrap();
    fs::write(&path, b"second").unwrap();
    store.acquire(&request).unwrap();
    assert_eq!(
        code(store.path(old.entry.uuid).unwrap_err()),
        ErrorCode::ContentUnavailable
    );
    fs::remove_file(path).unwrap();
    assert_eq!(
        code(store.acquire(&request).unwrap_err()),
        ErrorCode::SourceError
    );
    // A malformed persisted record is an integrity failure, not malformed caller input.
    fs::write(
        store.root().join(format!("index/{}/index.json", app.uuid)),
        b"{",
    )
    .unwrap();
    assert_eq!(
        code(store.application(app.uuid).unwrap_err()),
        ErrorCode::IntegrityError
    );
    // Filesystem failures retain a typed cause even through anyhow context.
    let io = fs::read(tmp.path().join("does-not-exist")).unwrap_err();
    assert_eq!(
        code(anyhow::Error::new(io).context("outer filesystem context")),
        ErrorCode::IoError
    );
}
