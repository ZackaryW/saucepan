//! CLI command definitions and direct core dispatch.
use crate::core::{Store, models::*};
use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

#[derive(Subcommand)]
pub(super) enum Command {
    /// Explicitly create a new encrypted store and its native key.
    Init,
    /// Persist a new app and print its stable proof (may be saved as .saucepanhash).
    Register {
        app: String,
        #[arg(long)]
        settings: Option<PathBuf>,
        #[arg(long)]
        filters: Option<PathBuf>,
    },
    /// Update the registered app's encrypted configuration using JSON files.
    Configure {
        #[arg(long)]
        settings: Option<PathBuf>,
        #[arg(long)]
        filters: Option<PathBuf>,
    },
    /// Acquire a declarative JSON recipe.
    Acquire { recipe: PathBuf },
    /// Return this app's touched, filtered entries.
    View,
    /// Verify a saved JSON view against the app's current authenticated record.
    Verify { view: PathBuf },
    /// Return the selected artifact's central directory, or null if absent.
    Path { artifact: String },
    /// Create a new independent mirror; occupied destinations are preserved.
    Mirror {
        artifact: String,
        destination: PathBuf,
    },
    /// Return a selected source's current and historical snapshot metadata.
    History { source: String },
    /// Read an exact retained snapshot without advancing its tracked source.
    Snapshot {
        source: String,
        snapshot: String,
        #[arg(long)]
        folder: Option<String>,
    },
    /// Resolve the single user-level executable path.
    SharedExecutable,
}

fn read<T: serde::de::DeserializeOwned>(path: PathBuf) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub(super) fn execute(store: &Store, context: &AppContext, command: Command) -> Result<Value> {
    Ok(match command {
        Command::Init => json!({"created": true}),
        Command::Register {
            app,
            settings,
            filters,
        } => serde_json::to_value(store.register(
            &app,
            settings.map(read).transpose()?.unwrap_or_default(),
            filters.map(read).transpose()?.unwrap_or_default(),
        )?)?,
        Command::Configure { settings, filters } => {
            let current = store.view(context)?;
            store.configure(
                context,
                settings.map(read).transpose()?.unwrap_or(current.settings),
                filters.map(read).transpose()?.unwrap_or(current.filters),
            )?;
            json!({"configured": true})
        }
        Command::Acquire { recipe } => {
            serde_json::to_value(store.acquire(context, &read::<Recipe>(recipe)?)?)?
        }
        Command::View => serde_json::to_value(store.view(context)?)?,
        Command::Verify { view } => {
            store.verify_view(context, &read(view)?)?;
            json!({"verified": true})
        }
        Command::Path { artifact } => {
            serde_json::to_value(store.artifact_path(context, &artifact)?)?
        }
        Command::Mirror {
            artifact,
            destination,
        } => {
            store.mirror(context, &artifact, &destination)?;
            json!({"directory": destination})
        }
        Command::History { source } => serde_json::to_value(store.source_state(context, &source)?)?,
        Command::Snapshot {
            source,
            snapshot,
            folder,
        } => serde_json::to_value(store.read_snapshot(context, &source, &snapshot, folder)?)?,
        Command::SharedExecutable => {
            serde_json::to_value(crate::core::platform::shared_executable()?)?
        }
    })
}
