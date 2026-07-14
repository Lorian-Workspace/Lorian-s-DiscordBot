//! Self-updater module for the bot.
//!
//! This module implements a secure self-update mechanism that:
//! - Fetches the latest release from GitHub
//! - Verifies the binary checksum
//! - Atomically replaces the current binary
//! - Restarts the process

mod github;
mod download;
mod replace;
mod state;

use std::sync::Arc;
use tokio::sync::RwLock;
use semver::Version;

pub use github::{Release, fetch_latest_release};
pub use download::download_and_verify;
pub use replace::atomic_replace;
pub use state::{UpdateState, load_state, save_state, clear_state};

use crate::config;

/// Global lock to prevent concurrent updates
static UPDATE_LOCK: std::sync::LazyLock<Arc<RwLock<()>>> = std::sync::LazyLock::new(|| Arc::new(RwLock::new(())));

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
                write!(f, "Checksum mismatch: expected {}, got {}", expected, actual)
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

    let latest_version = Version::parse(&release.tag_name.trim_start_matches('v'))
        .map_err(|e| UpdaterError::InvalidResponse(format!("Invalid version: {}", e)))?;

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

/// Apply an update
pub async fn apply_update(update: &UpdateInfo) -> Result<(), UpdaterError> {
    // Try to acquire lock
    let lock = UPDATE_LOCK.try_write().map_err(|_| UpdaterError::LockBusy)?;

    // Save state
    let state = UpdateState {
        version: update.version.clone(),
        started_at: chrono::Utc::now(),
    };
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Download and verify
    let current_exe = std::env::current_exe().map_err(|e| UpdaterError::FileIo(e.to_string()))?;
    let temp_path = current_exe.with_extension("part");

    download_and_verify(&update.asset_url, &update.checksum_url, &temp_path).await?;

    // Self-check
    self_check(&temp_path, &update.version).await?;

    // Atomic replace
    atomic_replace(&temp_path, &current_exe).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Clear state
    clear_state().map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Restart
    restart().map_err(|e| UpdaterError::ExecFailed(e.to_string()))?;

    // This line should never be reached
    drop(lock);
    Ok(())
}

/// Perform self-check on the new binary
async fn self_check(binary_path: &std::path::Path, expected_version: &Version) -> Result<(), UpdaterError> {
    use tokio::process::Command;
    use std::os::unix::fs::PermissionsExt;

    // Make executable
    let mut perms = std::fs::metadata(binary_path)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(binary_path, perms)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    // Run --version with timeout
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new(binary_path).arg("--version").output()
    )
    .await
    .map_err(|_| UpdaterError::SelfCheckFailed("Timeout".to_string()))?
    .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(UpdaterError::SelfCheckFailed("Non-zero exit code".to_string()));
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

/// Restart the process
fn restart() -> Result<(), std::io::Error> {
    use std::os::unix::process::CommandExt;

    let exe = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().collect();

    // exec replaces the current process
    Err(std::process::Command::new(exe).args(&args[1..]).exec())
}

/// Get the current version from Cargo.toml
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

/// Try to acquire the update lock (non-blocking)
pub fn try_acquire_lock() -> Option<tokio::sync::RwLockWriteGuard<'static, ()>> {
    UPDATE_LOCK.try_write().ok()
}
