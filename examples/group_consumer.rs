//! cargo run --example group_consumer -- ROOT GROUP_JSON [APP_UUID]
use saucepan::{
    core::{Store, policy::catalog::PolicyCatalog},
    models::lease::GroupRequest,
};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(
        (2..=3).contains(&args.len()),
        "expected ROOT GROUP_JSON [APP_UUID]"
    );
    let store = Store::open(std::path::Path::new(&args[0]))?;
    let request: GroupRequest = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let catalog = PolicyCatalog::default();
    let group = if let Some(app) = args.get(2) {
        // Supply a Credential to session() if this app opted into credentials.
        store
            .session(
                app.to_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid UUID"))?
                    .parse()?,
                None,
            )?
            .lease_group(&request, &catalog)?
    } else {
        store.lease_group(&request, &catalog)?
    };
    for (name, member) in &group.descriptor().members {
        println!("{name}: {}", member.directory.display());
        // Read or execute application logic while `group` remains in scope.
        std::fs::read_dir(&member.directory)?;
    }
    drop(group); // All retained output lifetimes end together.
    Ok(())
}
