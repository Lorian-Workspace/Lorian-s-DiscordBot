//! Update state persistence with transaction journal
//!
//! Security design:
//! - State file read: O_NOFOLLOW to prevent symlink redirection
//! - State file write: create_new + 0600 temp → sync → rename → dir sync
//! - After open, verify file is regular + owned by current uid + mode ≤ 0600
//! - Lock file: O_NOFOLLOW + regular/uid/mode validation (defense-in-depth)
//! - On startup: recover from crash during any phase

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

/// Verify opened file is a regular file owned by current uid with mode ≤ 0600.
/// Security: defense-in-depth after O_NOFOLLOW — catches exotic filesystem
/// behaviors or kernel bugs where O_NOFOLLOW is bypassed.
pub(crate) fn validate_regular_file(file: &File, path: &std::path::Path) -> std::io::Result<()> {
    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{:?} is not a regular file", path),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let uid = unsafe { libc::getuid() };
        if meta.uid() != uid {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{:?} owner {} != current uid {}", path, meta.uid(), uid),
            ));
        }
        let mode = meta.mode() & 0o777;
        if mode > 0o600 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{:?} mode 0o{:o} exceeds 0o600", path, mode),
            ));
        }
    }
    Ok(())
}

/// Phase of the update transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    /// Initial state - update started
    Started,
    /// Downloading new binary
    Downloading,
    /// Verifying checksum
    Verifying,
    /// Self-check running
    SelfChecking,
    /// Backup created
    BackedUp,
    /// New binary staged
    Staged,
    /// Ready to replace
    Replacing,
    /// Update complete
    Completed,
}

/// State of an in-progress update transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    /// Current phase of the update
    pub phase: UpdatePhase,
    /// Target version
    pub version: Version,
    /// When the update started
    pub started_at: DateTime<Utc>,
    /// Path to the original binary
    pub original_path: PathBuf,
    /// Path to the staged (downloaded) binary
    pub staged_path: PathBuf,
    /// Path to the backup binary
    pub backup_path: PathBuf,
}

/// Get the path to the state file
fn state_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    exe.with_extension("update_state.json")
}

/// Load update state from disk with O_NOFOLLOW protection
pub fn load_state() -> Option<UpdateState> {
    let path = state_path();
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .ok()?;
    validate_regular_file(&file, &path).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save update state to disk with secure file creation
///
/// Security: uses O_EXCL (create_new) on a temp file to guarantee we don't
/// overwrite an existing attacker-controlled file, then sync+rename+dirsync.
pub fn save_state(state: &UpdateState) -> std::io::Result<()> {
    let path = state_path();
    let json = serde_json::to_string_pretty(state)?;

    // Temp path alongside the final state file (same filesystem for atomic rename)
    let temp_path = path.with_extension("update_state.json.tmp");

    // Remove stale temp from prior crash — safe because same path we created
    let _ = std::fs::remove_file(&temp_path);

    // create_new (O_EXCL) guarantees we start with a fresh file — no symlink,
    // no hardlink reuse, no TOCTOU on pre-existing content
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temp_path)?;

    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    drop(file);

    // Atomic rename — replaces state.json atomically on POSIX
    std::fs::rename(&temp_path, &path)?;

    // Sync parent directory to persist the rename through crash
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
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

/// Reconcile state on startup after potential power loss / exec failure.
///
/// Recovery rules:
/// - No state file → no action (clean startup).
/// - Phase == Completed → clean update, clear state.
/// - Backup exists AND original missing → restore backup, clear state.
/// - Backup exists AND original exists → warn (possibly partial replace).
/// - Neither → warn, keep state for manual inspection.
pub fn reconcile_startup_state() -> std::io::Result<()> {
    let state = match load_state() {
        Some(s) => s,
        None => return Ok(()),
    };

    // Completed → new process started successfully after a prior update
    if state.phase == UpdatePhase::Completed {
        eprintln!("Cleaning up completed update state for v{}", state.version);
        return clear_state();
    }

    eprintln!(
        "WARNING: Found pending update state for v{} (phase: {:?})",
        state.version, state.phase
    );

    // Restore from backup if original binary is missing
    if state.backup_path.exists() && !state.original_path.exists() {
        eprintln!("Restoring original binary from backup...");
        if let Err(e) = std::fs::rename(&state.backup_path, &state.original_path) {
            eprintln!("ERROR: Failed to restore backup: {}", e);
            // Don't clear state — manual intervention needed
            return Err(e);
        }

        // Sync the restored file
        if let Ok(file) = File::open(&state.original_path) {
            let _ = file.sync_all();
        }
        if let Some(parent) = state.original_path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        clear_state()?;
        eprintln!("Backup restored successfully — original binary recovered");
    } else if state.backup_path.exists() {
        // Both original and backup exist — partial replace mid-step
        eprintln!(
            "WARNING: Both original and backup exist after incomplete update.\n  \
             Original: {:?}\n  Backup: {:?}\n  \
             Manual review recommended before removing the backup.",
            state.original_path, state.backup_path
        );
    } else {
        // No backup — can't auto-recover
        eprintln!(
            "WARNING: Cannot auto-recover. No backup found.\n  \
             Original: {:?}\n  Staged: {:?}\n  \
             Manual intervention required.",
            state.original_path, state.staged_path
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_state.json");

        let state = UpdateState {
            phase: UpdatePhase::Downloading,
            version: Version::new(1, 2, 3),
            started_at: Utc::now(),
            original_path: PathBuf::from("/tmp/original"),
            staged_path: PathBuf::from("/tmp/staged"),
            backup_path: PathBuf::from("/tmp/backup"),
        };

        // Save state
        let json = serde_json::to_string(&state).unwrap();
        std::fs::write(&state_path, json).unwrap();

        // Load state
        let loaded_json = std::fs::read_to_string(&state_path).unwrap();
        let loaded: UpdateState = serde_json::from_str(&loaded_json).unwrap();

        assert_eq!(state.phase, loaded.phase);
        assert_eq!(state.version, loaded.version);
    }

    #[test]
    fn test_phase_serialization() {
        let phase = UpdatePhase::Downloading;
        let json = serde_json::to_string(&phase).unwrap();
        assert_eq!(json, "\"downloading\"");

        let deserialized: UpdatePhase = serde_json::from_str(&json).unwrap();
        assert_eq!(phase, deserialized);
    }
}
