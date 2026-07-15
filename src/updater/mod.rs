//! Self-updater module for the bot.
//!
//! This module implements a secure self-update mechanism that:
//! - fetches the latest release from GitHub
//! - verifies the binary checksum
//! - atomically replaces the current binary
//! - restarts the process

mod download;
mod github;
mod replace;
mod state;

use semver::Version;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, MutexGuard};

use crate::config;
use download::download_and_verify;
use github::fetch_latest_release;
use replace::atomic_replace;
use state::{save_state, UpdateState};

pub use state::{clear_state, reconcile_startup_state};

/// Global lock to prevent concurrent updates (process-local).
static UPDATE_LOCK: std::sync::LazyLock<Arc<Mutex<()>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(())));

fn lock_file_path() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .with_extension("update.lock")
}

/// Open the cross-process lock file with O_NOFOLLOW + owner/mode validation.
/// Returns `None` when the lock file does not exist yet (caller should create it).
fn open_lock_file(path: &Path) -> Result<Option<File>, std::io::Error> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => {
            state::validate_regular_file(&file, path)?;
            Ok(Some(file))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn try_acquire_process_lock(lock: &Mutex<()>) -> Result<MutexGuard<'_, ()>, UpdaterError> {
    lock.try_lock().map_err(|_| UpdaterError::LockBusy)
}

fn try_lock_file(file: &File) -> Result<(), UpdaterError> {
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => {
            Err(UpdaterError::LockBusy)
        }
        _ => Err(UpdaterError::FileIo(format!(
            "Failed to lock update file: {err}"
        ))),
    }
}

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
    NoReleaseAvailable,
    InvalidResponse(String),
}

impl std::fmt::Display for UpdaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHubApi(e) => write!(f, "GitHub API error: {e}"),
            Self::Network(e) => write!(f, "Network error: {e}"),
            Self::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected {expected}, got {actual}")
            }
            Self::VersionMismatch { expected, actual } => {
                write!(f, "Version mismatch: expected {expected}, got {actual}")
            }
            Self::SelfCheckFailed(e) => write!(f, "Self-check failed: {e}"),
            Self::FileIo(e) => write!(f, "File I/O error: {e}"),
            Self::ExecFailed(e) => write!(f, "Exec failed: {e}"),
            Self::LockBusy => write!(f, "Update already in progress"),
            Self::NoReleaseAvailable => write!(f, "No release available"),
            Self::InvalidResponse(e) => write!(f, "Invalid response: {e}"),
        }
    }
}

impl std::error::Error for UpdaterError {}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: Version,
    pub asset_url: String,
    pub checksum_url: String,
}

pub async fn check_for_update() -> Result<Option<UpdateInfo>, UpdaterError> {
    let current_version = current_version();
    let release = fetch_latest_release().await?;

    let tag_version = release.tag_name.trim_start_matches('v');
    let latest_version = Version::parse(tag_version)
        .map_err(|e| UpdaterError::InvalidResponse(format!("Invalid version: {e}")))?;

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
    let checksum_url = format!("{asset_url}.sha256");

    Ok(Some(UpdateInfo {
        version: latest_version,
        asset_url,
        checksum_url,
    }))
}

pub async fn apply_update(update: &UpdateInfo) -> Result<(), UpdaterError> {
    let process_lock = try_acquire_process_lock(UPDATE_LOCK.as_ref())?;

    let lock_path = lock_file_path();
    let cross_lock_file = match open_lock_file(&lock_path)
        .map_err(|e| UpdaterError::FileIo(format!("Failed to access lock file: {e}")))?
    {
        Some(file) => file,
        None => {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&lock_path)
                .map_err(|e| UpdaterError::FileIo(format!("Failed to create lock file: {e}")))?;
            state::validate_regular_file(&file, &lock_path)
                .map_err(|e| UpdaterError::FileIo(e.to_string()))?;
            file
        }
    };
    try_lock_file(&cross_lock_file)?;

    let original_exe = std::env::current_exe().map_err(|e| UpdaterError::FileIo(e.to_string()))?;
    let temp_path = original_exe.with_extension("part");
    let backup_path = original_exe.with_extension("bak");

    let mut state = UpdateState {
        phase: state::UpdatePhase::Started,
        version: update.version.clone(),
        started_at: chrono::Utc::now(),
        original_path: original_exe.clone(),
        staged_path: temp_path.clone(),
        backup_path: backup_path.clone(),
    };
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    state.phase = state::UpdatePhase::Downloading;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    if let Err(err) = download_and_verify(&update.asset_url, &update.checksum_url, &temp_path).await
    {
        let _ = std::fs::remove_file(&temp_path);
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(err);
    }

    state.phase = state::UpdatePhase::SelfChecking;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    if let Err(err) = self_check(&temp_path, &update.version).await {
        let _ = std::fs::remove_file(&temp_path);
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(err);
    }

    state.phase = state::UpdatePhase::Replacing;
    save_state(&state).map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    if let Err(err) = atomic_replace(&temp_path, &original_exe) {
        let _ = std::fs::remove_file(&temp_path);
        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(err);
    }

    if let Err(err) = restart(&original_exe) {
        if backup_path.exists() {
            if let Err(restore_err) = std::fs::rename(&backup_path, &original_exe) {
                eprintln!("CRITICAL: Failed to restore backup: {restore_err}");
                drop(cross_lock_file);
                drop(process_lock);
                return Err(UpdaterError::ExecFailed(err.to_string()));
            }

            if let Ok(file) = File::open(&original_exe) {
                let _ = file.sync_all();
            }
            if let Some(parent) = original_exe.parent() {
                if let Ok(dir) = File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }

        let _ = clear_state();
        drop(cross_lock_file);
        drop(process_lock);
        return Err(UpdaterError::ExecFailed(err.to_string()));
    }

    drop(cross_lock_file);
    drop(process_lock);
    Ok(())
}

#[derive(Debug)]
struct CapturedProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn run_command_with_timeout(
    program: &Path,
    args: &[OsString],
    timeout: Duration,
) -> Result<CapturedProcessOutput, UpdaterError> {
    let mut child = tokio::process::Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?;

    let mut stdout = child.stdout.take().ok_or_else(|| {
        UpdaterError::SelfCheckFailed("failed to capture child stdout".to_string())
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| {
        UpdaterError::SelfCheckFailed("failed to capture child stderr".to_string())
    })?;

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).await.map(|_| buf)
    });
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        stderr.read_to_end(&mut buf).await.map(|_| buf)
    });

    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => result.map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?,
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(UpdaterError::SelfCheckFailed("Timeout".to_string()));
        }
    };

    let stdout = stdout_task
        .await
        .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?
        .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?;
    let stderr = stderr_task
        .await
        .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?
        .map_err(|e| UpdaterError::SelfCheckFailed(e.to_string()))?;

    Ok(CapturedProcessOutput {
        status,
        stdout,
        stderr,
    })
}

async fn self_check(binary_path: &Path, expected_version: &Version) -> Result<(), UpdaterError> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = std::fs::metadata(binary_path)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(binary_path, perms)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    let output = run_command_with_timeout(
        binary_path,
        &[OsString::from("--version")],
        Duration::from_secs(10),
    )
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(UpdaterError::SelfCheckFailed(format!(
            "Non-zero exit code: {}",
            stderr.trim()
        )));
    }

    let version_output = String::from_utf8_lossy(&output.stdout);
    let version_str = version_output.trim().trim_start_matches('v');

    let actual_version = Version::parse(version_str)
        .map_err(|e| UpdaterError::SelfCheckFailed(format!("Invalid version output: {e}")))?;

    if actual_version != *expected_version {
        return Err(UpdaterError::VersionMismatch {
            expected: expected_version.to_string(),
            actual: actual_version.to_string(),
        });
    }

    Ok(())
}

fn restart(exe_path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::process::CommandExt;

    let args: Vec<OsString> = std::env::args_os().collect();
    Err(std::process::Command::new(exe_path).args(&args[1..]).exec())
}

pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn current_version_parses() {
        assert!(!current_version().to_string().is_empty());
    }

    #[test]
    fn updater_error_display_mentions_lock_busy() {
        let msg = UpdaterError::LockBusy.to_string();
        assert!(msg.contains("already in progress"));
    }

    #[test]
    fn process_lock_reports_busy_when_held() {
        let lock = Mutex::new(());
        let _guard = try_acquire_process_lock(&lock).unwrap();

        assert!(matches!(
            try_acquire_process_lock(&lock),
            Err(UpdaterError::LockBusy)
        ));
    }

    #[test]
    fn open_lock_file_rejects_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let real_path = temp_dir.path().join("real.lock");
        let link_path = temp_dir.path().join("update.lock");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&real_path)
            .unwrap();
        drop(file);
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        assert!(open_lock_file(&link_path).is_err());
    }

    #[test]
    fn file_lock_reports_busy_when_already_held() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("update.lock");

        let first = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
            .unwrap();
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();

        try_lock_file(&first).unwrap();

        assert!(matches!(
            try_lock_file(&second),
            Err(UpdaterError::LockBusy)
        ));

        unsafe {
            libc::flock(first.as_raw_fd(), libc::LOCK_UN);
        }
    }

    #[tokio::test]
    async fn run_command_with_timeout_kills_and_reaps_child() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("child.pid");
        let script = "printf '%s' $$ > \"$1\"; trap 'exit 0' TERM; while :; do sleep 1; done";

        let err = run_command_with_timeout(
            Path::new("/bin/sh"),
            &[
                OsString::from("-c"),
                OsString::from(script),
                OsString::from("sh"),
                pid_file.as_os_str().to_os_string(),
            ],
            Duration::from_millis(200),
        )
        .await
        .unwrap_err();

        assert_eq!(err.to_string(), "Self-check failed: Timeout");

        let pid: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        let signal_result = unsafe { libc::kill(pid, 0) };
        let os_error = std::io::Error::last_os_error();

        assert_eq!(signal_result, -1);
        assert_eq!(os_error.raw_os_error(), Some(libc::ESRCH));
    }
}
