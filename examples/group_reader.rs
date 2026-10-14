//! Child of `saucepan run-group`; Saucepan retains ownership during this program.
fn main() -> anyhow::Result<()> {
    let file = std::env::var_os("SAUCEPAN_RESOURCE_GROUP_FILE")
        .ok_or_else(|| anyhow::anyhow!("launch through saucepan run-group"))?;
    let descriptor: serde_json::Value = serde_json::from_slice(&std::fs::read(file)?)?;
    let members = descriptor["members"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("invalid descriptor"))?;
    for (name, member) in members {
        let directory = member["directory"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing directory"))?;
        println!("{name}: {directory}");
        for entry in std::fs::read_dir(directory)? {
            println!("  {}", entry?.file_name().to_string_lossy());
        }
    }
    Ok(())
}
