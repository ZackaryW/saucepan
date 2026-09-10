use saucepan::core::{Store, models::*};

#[test]
fn library_consumer_acquires_without_cli_or_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    std::fs::create_dir(&input).unwrap();
    std::fs::write(input.join("asset"), b"content").unwrap();
    let store = Store::create_test(temp.path().join("store"), [7; 32]).unwrap();
    let app = AppContext::authenticated(
        store
            .register("consumer", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    let recipe = Recipe {
        source: Source::Local { path: input },
        folder: None,
        commit: None,
    };
    let acquired = store.acquire(&app, &recipe).unwrap();
    assert_eq!(
        std::fs::read(acquired.directory.join("asset")).unwrap(),
        b"content"
    );
    assert_eq!(
        store.artifact_path(&app, &acquired.artifact.id).unwrap(),
        Some(acquired.directory)
    );
    store.verify_view(&app, &store.view(&app).unwrap()).unwrap();
}

#[test]
fn filters_select_touched_entries_without_denying_acquisition() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("asset");
    std::fs::write(&input, "content").unwrap();
    let store = Store::create_test(temp.path().join("store"), [8; 32]).unwrap();
    let app = AppContext::authenticated(
        store
            .register(
                "a",
                AppSettings::default(),
                Filters {
                    providers: ["git".into()].into(),
                    ..Filters::default()
                },
            )
            .unwrap(),
    );
    let other = AppContext::authenticated(
        store
            .register("b", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    let acquired = store
        .acquire(
            &app,
            &Recipe {
                source: Source::Local { path: input },
                folder: None,
                commit: None,
            },
        )
        .unwrap();
    assert!(store.view(&app).unwrap().entries.is_empty());
    store
        .configure(&app, AppSettings::default(), Filters::default())
        .unwrap();
    assert!(
        store
            .view(&app)
            .unwrap()
            .entries
            .contains_key(&acquired.artifact.id)
    );
    assert!(store.view(&other).unwrap().entries.is_empty());
}
