fn main() {
    println!("cargo:rerun-if-env-changed=SAUCEPAN_BUILD_ID");
    let id = std::env::var("SAUCEPAN_BUILD_ID").unwrap_or_else(|_| "dev".into());
    assert!(
        !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "invalid build ID"
    );
    println!("cargo:rustc-env=SAUCEPAN_BUILD_ID={id}");
}
