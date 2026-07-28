use std::fs;
use tempfile::TempDir;

/// Read `.saucepan/index.json` from a workspace as raw text.
pub fn read_index_text(workspace: &TempDir) -> String {
    fs::read_to_string(workspace.path().join(".saucepan/index.json")).unwrap()
}

/// Read and parse `.saucepan/index.json` from a workspace.
pub fn read_index_json(workspace: &TempDir) -> serde_json::Value {
    serde_json::from_str(&read_index_text(workspace)).unwrap()
}
