//! Self-updater module for the bot.
//!
//! This module implements a secure self-update mechanism that:
//! - Fetches the latest release from GitHub
//! - Verifies the binary checksum
//! - Atomically replaces the current binary
//! - Restarts the process

mod download;
mod github;
mod replace;
mod state;

use semver::Version;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use download::download_and_verify;
pub use github::fetch_latest_release;
pub use replace::atomic_replace;
pub use state::{
    clear_state, load_state, reconcile_startup_state, save_state, UpdatePhase, UpdateState,
};

use crate::config;

/// Global lock to prevent concurrent updates (process-local)
static UPDATE_LOCK: std::sync::LazyLock<Arc<Mutex<()>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(())));

/// Cross-process lock file path
fn lock_file_path() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .with_extension("update.lock")
}

/// Open the cross-process lock file with O_NOFOLLOW + owner/mode validation.
/// Returns `None` when the lock file does not exist yet (caller should create it).
fn open_lock_file(path: &std::path::Path) -> Result<Option<File>, std::io::Error> {
    use std::os::unix::fs::OpenOptionsExt;

    if !path.exists() {
        return Ok(None);
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;

    state::validate_regular_file(&file, path)?;
    Ok(Some(file))
}

/// Error types for the updater
#[derive(Debug)]
pub enum UpdaterError {
    GitHubApi(String),
    Network(String),
    ChecksumMismatch { expected: String, actual: String },
    VersionMismatch { expected: String, actual: String },
    SelfCheckFailed(String),
    FileIo(String),
    ExecFailed(String),
    LockBusy,
    AlreadyUpToDate,
    NoReleaseAvailable,
    InvalidResponse(String),
}

impl std::fmt::Display for UpdaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHubApi(e) => write!(f, "GitHub API error: {}", e),
            Self::Network(e) => write!(f, "Network error: {}", e),
            Self::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "Checksum mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::VersionMismatch { expected, actual } => {
                write!(f, "Version mismatch: expected {}, got {}", expected, actual)
            }
            Self::SelfCheckFailed(e) => write!(f, "Self-check failed: {}", e),
            Self::FileIo(e) => write!(f, "File I/O error: {}", e),
            Self::ExecFailed(e) => write!(f, "Exec failed: {}", e),
            Self::LockBusy => write!(f, "Update already in progress"),
            Self::AlreadyUpToDate => write!(f, "Already up to date"),
            Self::NoReleaseAvailable => write!(f, "No release available"),
            Self::InvalidResponse(e) => write!(f, "Invalid response: {}", e),
        }
    }
}

impl std::error::Error for UpdaterError {}

/// Information about an available update
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: Version,
    pub asset_url: String,
    pub checksum_url: String,
    pub release_notes: String,
}

/// Check if an update is available
pub async fn check_for_update() -> Result<Option<UpdateInfo>, UpdaterError> {
    let current_version = current_version();
    let release = fetch_latest_release().await?;

    // Strict stable semver: must be vX.Y.Z with no prerelease/build metadata
    let tag_version = release.tag_name.trim_start_matches('v');
    let latest_version = Version::parse(tag_version)
        .map_err(|e| UpdaterError::InvalidResponse(format!("Invalid version: {}", e)))?;

    // Reject prerelease and build metadata
    if !latest_version.pre.is_empty() || !latest_version.build.is_empty() {
        return Err(UpdaterError::NoReleaseAvailable);
    }

    if latest_version <= current_version {
        return Ok(None);
    }

    let asset_url = format!(
        "https://github.com/{}/releases/download/{}/{}-{}",
        config::GITHUB_REPO,
        release.tag_name,
        config::ASSET_BASE_NAME,
        config::TARGET_TRIPLE
    );

    let checksum_url = format!("{}.sha256", asset_url);

    Ok(Some(UpdateInfo {
        version: latest_version,
        asset_url,
        checksum_url,
        release_notes: release.body.unwrap_or_default(),
    }))
}

/// Apply an update (acquires both process and cross-process locks)
pub async fn apply_update(update: &UpdateInfo) -> Result<(), UpdaterError> {
    // Try to acquire process-local lock
    let process_lock = UPDATE_LOCK.try_lock().map_err(|_| UpdaterError::LockBusy)?;

    // Try to acquire cross-process lock
    let lock_path = lock_file_path();
    let cross_lock_file = {
        // Try existing lock file first (with O_NOFOLLOW validation)
        let existing = open_lock_file(&lock_path)
            .map_err(|e| UpdaterError::FileIo(format!("Failed to access lock file: {}", e)))?;

        match existing {
            Some(file) => file,
            None => {
                // Create new lock file with O_NOFOLLOW + create_new
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&lock_path)
                    .map_err(|e| {
                        UpdaterError::FileIo(format!("Failed to create lock file: {}", e))
                    })?;
                state::validate_regular_file(&file, &lock_path)
                    .map_err(|e| UpdaterError::FileIo(e.to_string()))?;
                file
            }
        }
    };

    if unsafe { libc::flock(cross_lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) != 0 } {
        drop(process_lock);
        return Err(UpdaterError::LockBusy);
    }

    // Capture original exe path BEFORE any rename
    let original_exe = std::env::current_exe().map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    let temp_path = original_exe.with_extension("part");
    let backup_path = original_exe.with_extension("bak");

    // Initialize state with Started phase
    let mut state = UpdateState {
        phase: state::UpdatePhase::Started,
        version: update.version.clone(),
        started_at: chrono::Utc::now(),
        original_path: original_exe.clone(),
        staged_path: temp_path.clone(),
        backup_path: backup_path.clone(),
    };
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Download and verify
    state.phase = state::UpdatePhase::Downloading;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    if let Err(e) = download_and_verify(&update.asset_url, &update.checksum_url, &temp_path).await {
        let _ = std::fs::remove_file(&temp_path);
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(e);
    }

    // Self-check
    state.phase = state::UpdatePhase::SelfChecking;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    if let Err(e) = self_check(&temp_path, &update.version).await {
        let _ = std::fs::remove_file(&temp_path);
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(e);
    }

    // Backup original
    state.phase = state::UpdatePhase::BackedUp;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Atomic replace
    state.phase = state::UpdatePhase::Replacing;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    if let Err(e) = atomic_replace(&temp_path, &original_exe) {
        let _ = std::fs::remove_file(&temp_path);
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(UpdaterError::FileIo(e.to_string()));
    }

    // Restart with original exe path
    if let Err(e) = restart(&original_exe) {
        // Exec failed — attempt rollback. rename atomically replaces target,
        // so no need to remove the original first.
        if backup_path.exists() {
            if let Err(restore_err) = std::fs::rename(&backup_path, &original_exe) {
                eprintln!("CRITICAL: Failed to restore backup: {}", restore_err);
                drop(cross_lock_file);
                drop(process_lock);
                return Err(UpdaterError::ExecFailed(e.to_string()));
            }
            // fsync after restore
            if let Ok(file) = File::open(&original_exe) {
                let _ = file.sync_all();
            }
            if let Some(parent) = original_exe.parent() {
                if let Ok(dir) = File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }
        // Clear state after successful restore
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(UpdaterError::ExecFailed(e.to_string()));
    }

    // This line should never be reached
    // Note: state is NOT cleared here - it will be cleared after Discord ready in new process
    drop(cross_lock_file);
    drop(process_lock);
    Ok(())
}

/// Perform self-check on the new binary
async fn self_check(
    binary_path: &std::path::Path,
    expected_version: &Version,
) -> Result<(), UpdaterError> {
    use std::os::unix::fs::PermissionsExt;
    use tokio::process::Command;

    // Make executable
    let mut perms = std::fs::metadata(binary_path)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(binary_path, perms)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Run --version with timeout and kill_on_drop
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new(binary_path)
            .arg("--version")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| UpdaterError::SelfCheckFailed("Timeout".to_string()))?
    .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(UpdaterError::SelfCheckFailed(
            "Non-zero exit code".to_string(),
        ));
    }

    let version_output = String::from_utf8_lossy(&output.stdout);
    let version_str = version_output.trim().trim_start_matches('v');

    let actual_version = Version::parse(version_str)
        .map_err(|e| UpdaterError::SelfCheckFailed(format!("Invalid version output: {}", e)))?;

    if actual_version != *expected_version {
        return Err(UpdaterError::VersionMismatch {
            expected: expected_version.to_string(),
            actual: actual_version.to_string(),
        });
    }

    Ok(())
}

/// Restart the process with the original exe path
fn restart(exe_path: &std::path::Path) -> Result<(), std::io::Error> {
    use std::os::unix::process::CommandExt;

    let args: Vec<String> = std::env::args().collect();

    // exec replaces the current process
    Err(std::process::Command::new(exe_path).args(&args[1..]).exec())
}

/// Get the current version from Cargo.toml
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_current_version_parses() {
        let version = current_version();
        let version_str = version.to_string();
        assert!(!version_str.is_empty());
        assert!(version_str.contains('.'));
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 1, 0);
        let v3 = Version::new(2, 0, 0);

        assert!(v2 > v1);
        assert!(v3 > v2);
        assert!(v1 < v2);
        assert!(v1 < v3);
    }

    #[test]
    fn test_update_info_clone() {
        let info = UpdateInfo {
            version: Version::new(1, 0, 0),
            asset_url: "https://example.com/binary".to_string(),
            checksum_url: "https://example.com/checksum".to_string(),
            release_notes: "Test release".to_string(),
        };

        let cloned = info.clone();
        assert_eq!(info.version, cloned.version);
        assert_eq!(info.asset_url, cloned.asset_url);
        assert_eq!(info.checksum_url, cloned.checksum_url);
        assert_eq!(info.release_notes, cloned.release_notes);
    }

    #[test]
    fn test_updater_error_display() {
        let err = UpdaterError::GitHubApi("404".to_string());
        assert!(format!("{}", err).contains("GitHub API error"));

        let err = UpdaterError::Network("timeout".to_string());
        assert!(format!("{}", err).contains("Network error"));

        let err = UpdaterError::ChecksumMismatch {
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        assert!(format!("{}", err).contains("Checksum mismatch"));

        let err = UpdaterError::LockBusy;
        assert!(format!("{}", err).contains("Update already in progress"));
    }

    #[test]
    fn test_state_persistence() {
        use std::path::PathBuf;

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_state.json");

        let state = UpdateState {
            phase: state::UpdatePhase::Downloading,
            version: Version::new(1, 2, 3),
            started_at: chrono::Utc::now(),
            original_path: PathBuf::from("/tmp/original"),
            staged_path: PathBuf::from("/tmp/staged"),
            backup_path: PathBuf::from("/tmp/backup"),
        };

        let json = serde_json::to_string(&state).unwrap();
        fs::write(&state_path, json).unwrap();

        let loaded_json = fs::read_to_string(&state_path).unwrap();
        let loaded: UpdateState = serde_json::from_str(&loaded_json).unwrap();

        assert_eq!(state.version, loaded.version);
        assert_eq!(state.phase, loaded.phase);
    }

    #[test]
    fn test_version_parsing() {
        assert!(Version::parse("1.0.0").is_ok());
        assert!(Version::parse("0.1.0").is_ok());
        assert!(Version::parse("1.2.3").is_ok());

        assert!(Version::parse("1.0").is_err());
        assert!(Version::parse("1").is_err());
        assert!(Version::parse("invalid").is_err());
    }

    #[test]
    fn test_version_with_v_prefix() {
        let version_str = "v1.2.3";
        let parsed = Version::parse(version_str.trim_start_matches('v'));
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), Version::new(1, 2, 3));
    }

    #[test]
    fn test_semver_prerelease_rejection() {
        let stable = Version::new(1, 0, 0);
        let prerelease = Version::parse("1.0.0-alpha").unwrap();

        assert!(prerelease < stable);
    }

    #[test]
    fn test_asset_url_format() {
        let repo = "Lorian-Workspace/Lorian-s-DiscordBot";
        let tag = "v1.0.0";
        let asset_name = "lorian-discord-bot-linux-x86_64";

        let url = format!(
            "https://github.com/{}/releases/download/{}/{}",
            repo, tag, asset_name
        );

        assert!(url.contains(repo));
        assert!(url.contains(tag));
        assert!(url.contains(asset_name));
        assert!(url.starts_with("https://github.com/"));
    }

    #[test]
    fn test_checksum_url_format() {
        let asset_url = "https://github.com/repo/releases/download/v1.0.0/binary";
        let checksum_url = format!("{}.sha256", asset_url);

        assert!(checksum_url.ends_with(".sha256"));
        assert!(checksum_url.starts_with(&asset_url));
    }

    #[test]
    fn test_temp_file_creation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("test.part");

        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path);

        assert!(file.is_ok());

        let file2 = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path);

        assert!(file2.is_err());
    }

    #[test]
    fn test_file_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().join("test_exec");

        let mut file = fs::File::create(&temp_path).unwrap();
        file.write_all(b"test").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&temp_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&temp_path, perms).unwrap();

            let metadata = fs::metadata(&temp_path).unwrap();
            let mode = metadata.permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    #[test]
    fn test_atomic_rename() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        fs::write(&source, "test content").unwrap();

        fs::rename(&source, &target).unwrap();

        assert!(!source.exists());
        assert!(target.exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "test content");
    }

    #[test]
    fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let original = temp_dir.path().join("binary");
        let backup = temp_dir.path().join("binary.bak");

        fs::write(&original, "original content").unwrap();

        fs::rename(&original, &backup).unwrap();

        assert!(!original.exists());
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), "original content");
    }

    #[test]
    fn test_lock_busy_error() {
        let err = UpdaterError::LockBusy;
        let msg = format!("{}", err);
        assert!(msg.contains("already in progress"));
    }

    #[test]
    fn test_no_release_error() {
        let err = UpdaterError::NoReleaseAvailable;
        let msg = format!("{}", err);
        assert!(msg.contains("No release available"));
    }
}
