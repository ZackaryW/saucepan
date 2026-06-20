mod bucket;
mod commands;
mod config;
mod error;
mod index;
mod sauce;
mod sources;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "saucepan", about = "Composable artifact manager")]
struct Cli {
    /// Path to the folder containing saucepan.toml
    root: PathBuf,
    /// Output results as newline-delimited JSON
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install a sauce by name
    Install { name: String },
    /// Update an installed sauce
    Update { name: String },
    /// List installed sauces
    List,
    /// Print the on-disk path of an installed sauce
    Path { name: String },
    /// Search buckets with a jq filter
    Search { filter: String },
    /// Manage registered buckets
    Bucket {
        #[command(subcommand)]
        action: BucketAction,
    },
    /// Print a target's JSON representation
    Cat {
        #[command(subcommand)]
        target: CatTarget,
    },
}

#[derive(Subcommand)]
enum BucketAction {
    /// Add a bucket URL
    Add { url: String },
    /// Remove a bucket by URL
    Remove { url: String },
    /// List registered buckets
    List,
}

#[derive(Subcommand)]
enum CatTarget {
    /// Dump the full local index
    Index,
    /// Dump the registered bucket list
    Buckets,
    /// Show a sauce entry from the index
    Sauce { name: String },
    /// Fetch and show a bucket.json
    Bucket { url: String },
}

fn exit_code_for(e: &anyhow::Error) -> i32 {
    if e.downcast_ref::<error::NotFound>().is_some() {
        1
    } else if e.downcast_ref::<error::SourceError>().is_some() {
        2
    } else if e.downcast_ref::<error::ConfigError>().is_some() {
        3
    } else if e.downcast_ref::<error::Conflict>().is_some() {
        4
    } else {
        5
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = &cli.root;
    let config = config::Config::load(root)?;
    let json = cli.json;

    match &cli.command {
        Cmd::Install { name } => commands::install::install(root, name, &config),
        Cmd::Update { name } => commands::update::update(root, name, &config),
        Cmd::List => commands::list::list(root, json),
        Cmd::Path { name } => commands::path::path(root, name),
        Cmd::Search { filter } => commands::search::search(root, filter, &config),
        Cmd::Bucket { action } => match action {
            BucketAction::Add { url } => commands::bucket::add(root, url),
            BucketAction::Remove { url } => commands::bucket::remove(root, url),
            BucketAction::List => commands::bucket::list(root, json),
        },
        Cmd::Cat { target } => match target {
            CatTarget::Index => commands::cat::cat_index(root),
            CatTarget::Buckets => commands::cat::cat_buckets(root),
            CatTarget::Sauce { name } => commands::cat::cat_sauce(root, name),
            CatTarget::Bucket { url } => commands::cat::cat_bucket(url),
        },
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(exit_code_for(&e));
    }
}
