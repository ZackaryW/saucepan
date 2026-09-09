//! Fresh command adapters over the coordinated native service.
pub(crate) mod commands;

use clap::{Parser, Subcommand};
use saucepan::{
    Error, ErrorKind,
    core::{Service, models::*},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    version,
    about = "Acquire scoped artifacts through the central Saucepan store"
)]
struct Cli {
    /// Application root, or an explicit management context for store commands.
    root: Option<PathBuf>,
    #[cfg(feature = "test-support")]
    #[arg(long, global = true, requires = "test_key_file")]
    test_store: Option<PathBuf>,
    /// File containing exactly 32 raw key bytes; available only in test builds.
    #[cfg(feature = "test-support")]
    #[arg(long, global = true, requires = "test_store")]
    test_key_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List currently inspectable installations belonging to this application.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Read installed metadata without exposing central store bookkeeping.
    Cat {
        #[command(subcommand)]
        target: CatTarget,
    },
    /// Recheck current authority and return opaque application cache tokens.
    Context,
    /// Create or replace an independent mirror at an authorized destination.
    Mirror {
        target: String,
        destination: PathBuf,
        /// Interpret target as an explicit materialization ID.
        #[arg(long)]
        materialization: bool,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Check and return a recorded mirror directory.
    MirrorPath {
        target: String,
        destination: PathBuf,
        #[arg(long)]
        materialization: bool,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Remove a recorded mirror while preserving its parent binding.
    Unmirror {
        target: String,
        destination: PathBuf,
        #[arg(long)]
        materialization: bool,
    },
    /// Materialize exact recorded content without a manifest or branch refresh.
    Materialize {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long)]
        artifact: String,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Return the directory of an explicit materialization.
    MaterializationPath {
        name: String,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Release an explicit materialization and its unchanged owned files.
    ReleaseMaterialization { name: String },
    /// Refresh an installed unit from its recorded recipe.
    Update {
        name: String,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Remove the caller's installation and its unchanged owned directory.
    Uninstall { name: String },
    /// Acquire a JSON recipe and install under its manifest name.
    Install {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Print the caller's installed directory, with optional content rechecking.
    Path {
        name: String,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Initialize, inspect, or manage the store without application TOML.
    Store {
        #[command(subcommand)]
        command: StoreCommand,
    },
    /// Acquire current content from an explicit JSON recipe; print its descriptor.
    Acquire {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
    /// Write an exact current/historical ZIP to stdout without advancing its ref.
    ReadArchive {
        #[arg(long)]
        recipe: PathBuf,
        #[arg(long)]
        artifact: String,
        #[arg(long, action = clap::ArgAction::Set)]
        verify: Option<bool>,
    },
}

#[derive(Subcommand)]
enum CatTarget {
    Index,
    Sauce { name: String },
}

#[derive(Subcommand)]
enum StoreCommand {
    Init,
    Check,
    /// Describe a supplied Git locator for recipe and source-scope construction.
    Source {
        locator: String,
    },
    /// Enroll an application from a JSON ApplicationRegistration document.
    Register {
        #[arg(long)]
        request: PathBuf,
    },
    /// Replace/rebind/reissue an application registration, or explicitly revoke it.
    UpdateRegistration {
        id: String,
        #[arg(long, required_unless_present = "revoke", conflicts_with = "revoke")]
        request: Option<PathBuf>,
        #[arg(long)]
        revoke: bool,
    },
    /// Replace a source's cache/verification policy using management scope.
    Policy {
        source_id: String,
        #[arg(long)]
        request: PathBuf,
    },
}

pub(crate) fn run() -> Result<()> {
    let args = Cli::parse();
    if matches!(
        args.command,
        Command::Store {
            command: StoreCommand::Init
        }
    ) {
        args.service(true)?.check_store()?;
        return commands::json(
            &serde_json::json!({"initialized": true, "schema_version": SCHEMA_VERSION}),
        );
    }
    let service = args.service(false)?;
    match &args.command {
        Command::List { .. } | Command::Cat { .. } | Command::Context => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            match &args.command {
                Command::Context => {
                    commands::json(&service.application_generation(root, marker.as_ref())?)
                }
                Command::Cat {
                    target: CatTarget::Sauce { name },
                } => commands::json(&service.unit_view(root, marker.as_ref(), name)?),
                command => {
                    let view = service.application_view(root, marker.as_ref())?;
                    match command {
                        Command::List { json: true } => commands::ndjson(&view.units),
                        Command::List { json: false } => {
                            let mut output = String::new();
                            for unit in view.units {
                                output.extend(
                                    unit.name
                                        .chars()
                                        .map(|c| if c.is_control() { ' ' } else { c }),
                                );
                                output.push('\n');
                            }
                            commands::bytes(output.as_bytes())
                        }
                        _ => commands::json(&view),
                    }
                }
            }
        }
        Command::Mirror {
            target,
            destination,
            materialization,
            verify,
        }
        | Command::MirrorPath {
            target,
            destination,
            materialization,
            verify,
        } => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            let target = binding_target(target, *materialization);
            let verification = VerificationRequest { content: *verify };
            if matches!(args.command, Command::Mirror { .. }) {
                commands::json(&service.mirror(
                    root,
                    marker.as_ref(),
                    &target,
                    destination,
                    &verification,
                )?)
            } else {
                let path = service.mirror_path(
                    root,
                    marker.as_ref(),
                    &target,
                    destination,
                    &verification,
                )?;
                let path = path
                    .to_str()
                    .ok_or_else(|| Error::new(ErrorKind::Config, "mirror path is not UTF-8"))?;
                commands::bytes(format!("{path}\n").as_bytes())
            }
        }
        Command::Unmirror {
            target,
            destination,
            materialization,
        } => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            service.remove_mirror(
                root,
                marker.as_ref(),
                &binding_target(target, *materialization),
                destination,
            )?;
            commands::json(&serde_json::json!({"removed": destination}))
        }
        Command::Uninstall { name } | Command::ReleaseMaterialization { name } => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            if matches!(args.command, Command::ReleaseMaterialization { .. }) {
                service.release_materialization(root, marker.as_ref(), name)?;
            } else {
                service.uninstall(root, marker.as_ref(), name)?;
            }
            commands::json(&serde_json::json!({"removed": name}))
        }
        Command::Update { name, verify } => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            commands::json(&service.update(
                root,
                marker.as_ref(),
                name,
                &VerificationRequest { content: *verify },
            )?)
        }
        Command::Path { name, verify } | Command::MaterializationPath { name, verify } => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            let path = if matches!(args.command, Command::MaterializationPath { .. }) {
                service.materialization_path(
                    root,
                    marker.as_ref(),
                    name,
                    &VerificationRequest { content: *verify },
                )?
            } else {
                service.path(
                    root,
                    marker.as_ref(),
                    name,
                    &VerificationRequest { content: *verify },
                )?
            };
            let path = path
                .to_str()
                .ok_or_else(|| Error::new(ErrorKind::Config, "installed path is not UTF-8"))?;
            commands::bytes(format!("{path}\n").as_bytes())
        }
        Command::Store { command } => {
            if matches!(command, StoreCommand::Check) {
                service.check_store()?;
                return commands::json(&serde_json::json!({"consistent": true}));
            }
            let store_root = args.store_root()?;
            let root = args.root.as_deref().unwrap_or(&store_root);
            let owner_context =
                std::fs::canonicalize(root).ok() == std::fs::canonicalize(&store_root).ok();
            let marker = marker(
                root,
                if owner_context {
                    "owner.saucepanhash"
                } else {
                    ".saucepanhash"
                },
            )?;
            service.validate_context(root, marker.as_ref())?;
            match command {
                StoreCommand::Source { locator } => commands::json(&service.describe_source(
                    root,
                    marker.as_ref(),
                    &SourceLocator {
                        backend: Backend::Git,
                        origin: locator.clone(),
                    },
                )?),
                StoreCommand::Register { request } => {
                    service.validate_management(root, marker.as_ref(), None)?;
                    let request: ApplicationRegistration = decode(request)?;
                    let credential =
                        service.register_application(root, marker.as_ref(), request)?;
                    // Explicit management output is the credential delivery channel;
                    // never print it in diagnostics or application query results.
                    commands::json(&credential)
                }
                StoreCommand::UpdateRegistration { id, request, .. } => {
                    service.validate_management(root, marker.as_ref(), None)?;
                    let replacement = request.as_ref().map(|path| decode(path)).transpose()?;
                    commands::json(&service.update_application(
                        root,
                        marker.as_ref(),
                        id,
                        replacement,
                    )?)
                }
                StoreCommand::Policy { source_id, request } => {
                    service.validate_management(root, marker.as_ref(), Some(source_id))?;
                    service.set_source_policy(
                        root,
                        marker.as_ref(),
                        source_id,
                        decode(request)?,
                    )?;
                    commands::json(&serde_json::json!({"updated": true}))
                }
                StoreCommand::Init | StoreCommand::Check => unreachable!("handled above"),
            }
        }
        Command::Install { recipe, verify }
        | Command::Acquire { recipe, verify }
        | Command::Materialize { recipe, verify, .. }
        | Command::ReadArchive { recipe, verify, .. } => {
            let root = args
                .root
                .as_deref()
                .ok_or_else(|| Error::new(ErrorKind::Config, "application root is required"))?;
            let marker = marker(root, ".saucepanhash")?;
            service.validate_context(root, marker.as_ref())?;
            commands::validate_preferences(root)?;
            let recipe = saucepan::core::parse_recipe(&read(
                recipe,
                1024 * 1024,
                "cannot read recipe document",
            )?)?;
            let verification = VerificationRequest { content: *verify };
            match &args.command {
                Command::Materialize { artifact, .. } => commands::json(&service.materialize(
                    root,
                    marker.as_ref(),
                    &recipe,
                    artifact,
                    &verification,
                )?),
                Command::Install { .. } => commands::json(&service.install(
                    root,
                    marker.as_ref(),
                    &recipe,
                    &verification,
                )?),
                Command::Acquire { .. } => commands::json(&service.acquire(
                    root,
                    marker.as_ref(),
                    &recipe,
                    &verification,
                )?),
                Command::ReadArchive { artifact, .. } => commands::bytes(
                    &service
                        .read_archive(root, marker.as_ref(), &recipe, artifact, &verification)?
                        .bytes,
                ),
                _ => unreachable!("application command matched"),
            }
        }
    }
}

impl Cli {
    fn store_root(&self) -> Result<PathBuf> {
        #[cfg(feature = "test-support")]
        if let Some(root) = &self.test_store {
            return Ok(root.clone());
        }
        dirs::home_dir()
            .map(|home| home.join(".saucepan"))
            .ok_or_else(|| Error::new(ErrorKind::Config, "user home is unavailable"))
    }
    fn service(&self, initialize: bool) -> Result<Service> {
        #[cfg(feature = "test-support")]
        if let (Some(root), Some(key_path)) = (&self.test_store, &self.test_key_file) {
            let bytes = zeroize::Zeroizing::new(read(
                key_path,
                32,
                "test key file must contain exactly 32 raw bytes",
            )?);
            let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                Error::new(
                    ErrorKind::Config,
                    "test key file must contain exactly 32 raw bytes",
                )
            })?;
            return if initialize {
                Service::initialize_test(root, key)
            } else {
                Service::open_test(root, key)
            };
        }
        if initialize {
            Service::initialize_user()
        } else {
            Service::open_user()
        }
    }
}

fn binding_target(target: &str, materialization: bool) -> BindingTarget {
    if materialization {
        BindingTarget::Materialization { id: target.into() }
    } else {
        BindingTarget::Unit {
            name: target.into(),
        }
    }
}

fn marker(root: &Path, name: &str) -> Result<Option<Marker>> {
    let path = root.join(name);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::new(
            ErrorKind::Authority,
            "cannot read application credential",
        )),
        Ok(_) => serde_json::from_slice(&read(
            &path,
            64 * 1024,
            "cannot read application credential",
        )?)
        .map(Some)
        .map_err(|_| Error::new(ErrorKind::Authority, "invalid application credential")),
    }
}
fn decode<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&read(path, 1024 * 1024, "cannot read request document")?)
        .map_err(|_| Error::new(ErrorKind::Config, "invalid request document"))
}
pub(crate) fn read(path: &Path, limit: usize, message: &str) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|_| Error::new(ErrorKind::Config, message))?;
    if !file
        .metadata()
        .map_err(|_| Error::new(ErrorKind::Config, message))?
        .is_file()
    {
        return Err(Error::new(ErrorKind::Config, message));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new(ErrorKind::Config, message))?;
    if bytes.len() > limit {
        return Err(Error::new(ErrorKind::Config, message));
    }
    Ok(bytes)
}
