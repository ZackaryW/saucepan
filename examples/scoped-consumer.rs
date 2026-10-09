//! cargo run --example scoped-consumer -- ROOT APP_UUID ENSURE_JSON RELATIVE_FILE [CREDENTIAL_FILE]
use anyhow::{Context, Result};
use saucepan::{
    core::Store,
    models::{
        claim::{EnsureRequest, security::Credential},
        store::VerificationMode,
    },
};
use std::{fs, path::Path};
fn read() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        matches!(args.len(), 4 | 5),
        "expected ROOT APP_UUID ENSURE_JSON RELATIVE_FILE [CREDENTIAL_FILE]"
    );
    let store = Store::open(Path::new(&args[0]))?;
    let credential = args
        .get(4)
        .map(|path| -> Result<Credential> {
            let bytes = zeroize::Zeroizing::new(fs::read(path)?);
            Ok(Credential::from_bytes(&bytes)?)
        })
        .transpose()?;
    let session = store.session(args[1].parse()?, credential)?;
    let request: EnsureRequest = serde_json::from_slice(&fs::read(&args[2])?)?;
    let result = session.ensure(&request, &Default::default())?;
    let lease = session.lease_view(&result.view, VerificationMode::Full)?;
    let path = saucepan::utils::path::beneath(lease.directory(), &args[3])?;
    let bytes = fs::read(path).context("read claimed package resource")?;
    println!(
        "{}",
        serde_json::json!({"claim":result.claim.uuid,"entry":lease.entry_uuid(),"bytes":bytes.len()})
    );
    // The native guard is still alive throughout the read, including error paths.
    Ok(())
}
fn main() {
    if let Err(error) = read() {
        eprintln!(
            "{}",
            serde_json::json!({"error":saucepan::error::Error::from_anyhow(&error)})
        );
        std::process::exit(1);
    }
}
