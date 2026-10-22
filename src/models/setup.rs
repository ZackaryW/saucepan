//! Versioned setup observations and durable preparation evidence.
use crate::models::{
    index::lifecycle::Phase,
    policy::{PolicyRecord, profile::ProfileExpansion},
    store::{Platform, Request},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Absent,
    Unmanaged,
    Ready,
    Pending,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub schema_version: u32,
    pub root: PathBuf,
    pub state: State,
    pub launcher_path: Option<PathBuf>,
    pub app: Option<Uuid>,
    pub claim: Option<Uuid>,
    pub output: Option<Uuid>,
    pub installed_version: Option<String>,
    pub operation: Option<Uuid>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub schema_version: u32,
    pub root: PathBuf,
    pub status: Phase,
    pub changed: bool,
    pub launcher_path: Option<PathBuf>,
    pub app: Option<Uuid>,
    pub claim: Option<Uuid>,
    pub output: Option<Uuid>,
    pub installed_version: Option<String>,
    pub operation: Uuid,
    pub lifecycle_operation: Option<Uuid>,
    pub backup_paths: Vec<PathBuf>,
    pub diagnostics: Vec<String>,
    pub retry_shim: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub store: Uuid,
    pub root: PathBuf,
    pub tag: String,
    pub platform: Platform,
    pub shim_digest: String,
    pub migrate_unmanaged: bool,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    Intent,
    BackedUp,
    Displaced,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    pub phase: MigrationPhase,
    pub source: PathBuf,
    pub backup: PathBuf,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema_version: u32,
    pub operation: Uuid,
    pub inputs: Inputs,
    pub fingerprint: String,
    pub profile: ProfileExpansion,
    pub filter: PolicyRecord,
    pub request: Request,
    pub lifecycle_operation: Uuid,
    pub shim: PathBuf,
    pub migration: Option<Migration>,
    pub result: Outcome,
}
