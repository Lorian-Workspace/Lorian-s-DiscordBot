//! Update state persistence with transaction journal.
//!
//! Security design:
//! - State file read: O_NOFOLLOW to prevent symlink redirection.
//! - State file write: create_new + 0600 temp → sync → rename → dir sync.
//! - After open, verify file is regular + owned by current uid + mode ≤ 0600.
//! - Lock file: O_NOFOLLOW + regular/uid/mode validation (defense-in-depth).
//! - On startup: recover from crash during any phase.

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Verify opened file is a regular file owned by current uid with mode ≤ 0600.
/// Security: defense-in-depth after O_NOFOLLOW — catches exotic filesystem
/// behaviors or kernel bugs where O_NOFOLLOW is bypassed.
pub(crate) fn validate_regular_file(file: &File, path: &Path) -> std::io::Result<()> {
    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{path:?} is not a regular file"),
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let uid = unsafe { libc::getuid() };
        if meta.uid() != uid {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{path:?} owner {} != current uid {}", meta.uid(), uid),
            ));
        }

        let mode = meta.mode() & 0o777;
        if mode > 0o600 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{path:?} mode 0o{mode:o} exceeds 0o600"),
            ));
        }
    }

    Ok(())
}

/// Phase of the update transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Started,
    Downloading,
    Verifying,
    SelfChecking,
    BackedUp,
    Staged,
    Replacing,
    Completed,
}

/// State of an in-progress update transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    pub phase: UpdatePhase,
    pub version: Version,
    pub started_at: DateTime<Utc>,
    pub original_path: PathBuf,
    pub staged_path: PathBuf,
    pub backup_path: PathBuf,
}

fn state_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    exe.with_extension("update_state.json")
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut temp_path = OsString::from(path.as_os_str());
    temp_path.push(".tmp");
    PathBuf::from(temp_path)
}

fn load_state_from_result(path: &Path) -> std::io::Result<Option<UpdateState>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    validate_regular_file(&file, path)?;

    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let state = serde_json::from_str(&content)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;

    Ok(Some(state))
}

fn save_state_to(path: &Path, state: &UpdateState) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(state)?;
    let temp_path = temp_path_for(path);

    let _ = std::fs::remove_file(&temp_path);

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temp_path)?;

    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    drop(file);

    std::fs::rename(&temp_path, path)?;

    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

pub fn save_state(state: &UpdateState) -> std::io::Result<()> {
    save_state_to(&state_path(), state)
}

fn clear_state_at(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

pub fn clear_state() -> std::io::Result<()> {
    clear_state_at(&state_path())
}

fn reconcile_startup_state_at(path: &Path) -> std::io::Result<()> {
    let state = match load_state_from_result(path)? {
        Some(state) => state,
        None => return Ok(()),
    };

    if state.phase == UpdatePhase::Completed {
        eprintln!("Cleaning up completed update state for v{}", state.version);
        return clear_state_at(path);
    }

    eprintln!(
        "WARNING: Found pending update state for v{} (phase: {:?})",
        state.version, state.phase
    );

    if state.backup_path.exists() && !state.original_path.exists() {
        eprintln!("Restoring original binary from backup...");
        std::fs::rename(&state.backup_path, &state.original_path)?;

        if let Ok(file) = File::open(&state.original_path) {
            let _ = file.sync_all();
        }
        if let Some(parent) = state.original_path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        clear_state_at(path)?;
        eprintln!("Backup restored successfully — original binary recovered");
    } else if state.backup_path.exists() {
        eprintln!(
            "WARNING: Both original and backup exist after incomplete update.\n  \
             Original: {:?}\n  Backup: {:?}\n  \
             Manual review recommended before removing the backup.",
            state.original_path, state.backup_path
        );
    } else {
        eprintln!(
            "WARNING: Cannot auto-recover. No backup found.\n  \
             Original: {:?}\n  Staged: {:?}\n  \
             Manual intervention required.",
            state.original_path, state.staged_path
        );
    }

    Ok(())
}

pub fn reconcile_startup_state() -> std::io::Result<()> {
    reconcile_startup_state_at(&state_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_state(base: &Path) -> UpdateState {
        UpdateState {
            phase: UpdatePhase::Replacing,
            version: Version::new(1, 2, 3),
            started_at: Utc::now(),
            original_path: base.join("lorian-discord-bot"),
            staged_path: base.join("lorian-discord-bot.part"),
            backup_path: base.join("lorian-discord-bot.bak"),
        }
    }

    #[test]
    fn save_state_to_is_atomic_and_private() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("update_state.json");
        let state = sample_state(temp_dir.path());

        save_state_to(&state_path, &state).unwrap();

        let loaded = load_state_from_result(&state_path).unwrap().unwrap();
        assert_eq!(loaded.phase, state.phase);
        assert_eq!(loaded.version, state.version);
        assert!(!temp_path_for(&state_path).exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            let mode = std::fs::metadata(&state_path).unwrap().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn load_state_from_result_rejects_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let real_path = temp_dir.path().join("real.json");
        let link_path = temp_dir.path().join("update_state.json");
        let state = sample_state(temp_dir.path());

        save_state_to(&real_path, &state).unwrap();
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        let err = load_state_from_result(&link_path).unwrap_err();
        assert!(
            matches!(err.raw_os_error(), Some(libc::ELOOP) | Some(libc::EMLINK))
                || err.kind() == std::io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn reconcile_startup_state_restores_backup_when_original_missing() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("update_state.json");
        let state = sample_state(temp_dir.path());

        std::fs::write(&state.backup_path, b"old-binary").unwrap();
        save_state_to(&state_path, &state).unwrap();

        reconcile_startup_state_at(&state_path).unwrap();

        assert_eq!(std::fs::read(&state.original_path).unwrap(), b"old-binary");
        assert!(!state.backup_path.exists());
        assert!(!state_path.exists());
    }
}
