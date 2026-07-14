//! Atomic file replacement with backup

use std::fs;
use std::path::Path;

use super::UpdaterError;

/// Atomically replace the current binary with a new one
///
/// This function:
/// 1. Renames current binary to .bak
/// 2. Renames new binary to current binary name
/// 3. On failure, attempts to restore from .bak
pub fn atomic_replace(new_binary: &Path, current_binary: &Path) -> Result<(), UpdaterError> {
    let backup_path = current_binary.with_extension("bak");

    // Verify new binary exists
    if !new_binary.exists() {
        return Err(UpdaterError::FileIo(
            "New binary does not exist".to_string(),
        ));
    }

    // Note: Cross-filesystem check omitted; rename will fail if on different filesystems

    // Rename current to backup
    if current_binary.exists() {
        fs::rename(current_binary, &backup_path)
            .map_err(|e| UpdaterError::FileIo(format!("Failed to create backup: {}", e)))?;
    }

    // Rename new to current
    if let Err(e) = fs::rename(new_binary, current_binary) {
        // Attempt to restore from backup
        if backup_path.exists() {
            let _ = fs::rename(&backup_path, current_binary);
        }
        return Err(UpdaterError::FileIo(format!(
            "Failed to replace binary: {}",
            e
        )));
    }

    // Make executable (Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(current_binary)
            .map_err(|e| UpdaterError::FileIo(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(current_binary, perms)
            .map_err(|e| UpdaterError::FileIo(format!("Failed to set permissions: {}", e)))?;
    }

    // Sync to disk
    if let Ok(file) = fs::File::open(current_binary) {
        let _ = file.sync_all();
    }
    if let Some(parent) = current_binary.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// Restore from backup if available
pub fn restore_backup(current_binary: &Path) -> Result<bool, UpdaterError> {
    let backup_path = current_binary.with_extension("bak");

    if !backup_path.exists() {
        return Ok(false);
    }

    // Remove current if it exists
    if current_binary.exists() {
        fs::remove_file(current_binary)
            .map_err(|e| UpdaterError::FileIo(format!("Failed to remove current binary: {}", e)))?;
    }

    // Rename backup to current
    fs::rename(&backup_path, current_binary)
        .map_err(|e| UpdaterError::FileIo(format!("Failed to restore from backup: {}", e)))?;

    Ok(true)
}
