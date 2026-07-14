//! Update state persistence

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use semver::Version;

/// State of an in-progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    pub version: Version,
    pub started_at: DateTime<Utc>,
}

/// Get the path to the state file
fn state_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    exe.with_extension("update_state.json")
}

/// Load update state from disk
pub fn load_state() -> Option<UpdateState> {
    let path = state_path();
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save update state to disk
pub fn save_state(state: &UpdateState) -> std::io::Result<()> {
    let path = state_path();
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(&path, json)?;

    // Sync to disk
    if let Ok(file) = std::fs::File::open(&path) {
        let _ = file.sync_all();
    }
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// Clear update state from disk
pub fn clear_state() -> std::io::Result<()> {
    let path = state_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
