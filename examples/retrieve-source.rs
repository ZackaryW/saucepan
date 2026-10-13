//! Inspect source material without publishing an acquired output.
use saucepan::{
    core::retrieval::Retriever,
    models::retrieval::{Ownership, RetrieveRequest},
};

fn main() -> anyhow::Result<()> {
    let input = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: retrieve-source REQUEST.json"))?;
    let request: RetrieveRequest = serde_json::from_slice(&std::fs::read(input)?)?;
    let retrieved = Retriever::retrieve(&request)?;
    let metadata = std::fs::metadata(retrieved.source_path())?;
    if matches!(retrieved.descriptor().ownership, Ownership::Caller) {
        println!("{}", serde_json::to_string_pretty(retrieved.descriptor())?);
    } else {
        println!(
            "inspected temporary {:?}; directory={}",
            retrieved.descriptor().kind,
            metadata.is_dir()
        );
    }
    Ok(())
}
