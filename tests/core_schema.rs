use saucepan::core::{Store, models::*};
use serde::{Serialize, de::DeserializeOwned};

fn roundtrip<T: Serialize + DeserializeOwned>(value: &T) {
    let json = serde_json::to_value(value).unwrap();
    let decoded: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), json);
}

#[test]
fn public_documents_roundtrip_without_exposing_internal_index_records() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("asset");
    std::fs::write(&input, "content").unwrap();
    let recipe = Recipe {
        source: Source::Local { path: input },
        folder: None,
        commit: None,
    };
    let store = Store::create_test(temp.path().join("store"), [9; 32]).unwrap();
    let proof = store
        .register("app", AppSettings::default(), Filters::default())
        .unwrap();
    roundtrip(&proof);
    let app = AppContext::authenticated(proof);
    roundtrip(&app);
    roundtrip(&recipe);
    let acquired = store.acquire(&app, &recipe).unwrap();
    roundtrip(&acquired);
    roundtrip(
        &store
            .source_state(&app, &acquired.artifact.source_id)
            .unwrap()
            .unwrap(),
    );
    let view = store.view(&app).unwrap();
    roundtrip(&view);
    let json = serde_json::to_value(view).unwrap();
    let keys: Vec<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["app", "entries", "filters", "settings", "version"]);
}

#[test]
fn recipes_accept_three_providers_and_reject_policy_overrides() {
    for source in [
        serde_json::json!({"provider":"git", "origin":"https://example.com/repo.git", "reference":"main"}),
        serde_json::json!({"provider":"url", "url":"https://example.com/asset.zip", "download":{"format":"zip"}}),
        serde_json::json!({"provider":"local", "path":"assets"}),
    ] {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({"source":source})).unwrap();
        roundtrip(&recipe);
        for field in ["snapshot_enabled", "scopes", "grants", "settings"] {
            let mut json = serde_json::to_value(&recipe).unwrap();
            json[field] = serde_json::json!(true);
            assert!(serde_json::from_value::<Recipe>(json).is_err());
        }
    }
    assert!(
        serde_json::from_value::<Recipe>(serde_json::json!({"source":{"provider":"unknown"}}))
            .is_err()
    );
    assert!(serde_json::from_value::<Recipe>(serde_json::json!({"source":{"provider":"git", "origin":"https://example.com/repo.git"}})).is_err());
}
