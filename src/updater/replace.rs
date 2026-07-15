//! Atomic file replacement with backup.

use std::fs;
use std::path::Path;

use super::UpdaterError;

fn rename_path(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

fn atomic_replace_with<F>(
    new_binary: &Path,
    current_binary: &Path,
    mut rename: F,
) -> Result<(), UpdaterError>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let backup_path = current_binary.with_extension("bak");

    if !new_binary.exists() {
        return Err(UpdaterError::FileIo(
            "New binary does not exist".to_string(),
        ));
    }

    if current_binary.exists() {
        rename(current_binary, &backup_path)
            .map_err(|e| UpdaterError::FileIo(format!("Failed to create backup: {e}")))?;
    }

    if let Err(replace_err) = rename(new_binary, current_binary) {
        if backup_path.exists() {
            rename(&backup_path, current_binary).map_err(|restore_err| {
                UpdaterError::FileIo(format!(
                    "Failed to replace binary: {replace_err}; backup restore also failed: {restore_err}"
                ))
            })?;
        }

        return Err(UpdaterError::FileIo(format!(
            "Failed to replace binary: {replace_err}"
        )));
    }

    Ok(())
}

/// Atomically replace the current binary with a new one.
///
/// This function:
/// 1. Renames current binary to `.bak`
/// 2. Renames new binary to current binary name
/// 3. On failure, restores `.bak` back to the original path
pub fn atomic_replace(new_binary: &Path, current_binary: &Path) -> Result<(), UpdaterError> {
    atomic_replace_with(new_binary, current_binary, rename_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(current_binary)
            .map_err(|e| UpdaterError::FileIo(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(current_binary, perms)
            .map_err(|e| UpdaterError::FileIo(format!("Failed to set permissions: {e}")))?;
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn atomic_replace_with_restores_backup_when_second_rename_fails() {
        let temp_dir = TempDir::new().unwrap();
        let current = temp_dir.path().join("lorian-discord-bot");
        let new_binary = temp_dir.path().join("lorian-discord-bot.part");

        std::fs::write(&current, b"old-binary").unwrap();
        std::fs::write(&new_binary, b"new-binary").unwrap();

        let mut call_count = 0usize;
        let err = atomic_replace_with(&new_binary, &current, |from, to| {
            call_count += 1;
            if call_count == 2 {
                return Err(std::io::Error::other("simulated replace failure"));
            }

            std::fs::rename(from, to)
        })
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Failed to replace binary: simulated replace failure"));
        assert_eq!(std::fs::read(&current).unwrap(), b"old-binary");
        assert_eq!(std::fs::read(&new_binary).unwrap(), b"new-binary");
        assert!(!current.with_extension("bak").exists());
    }
}
