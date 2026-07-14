//! Central configuration constants for the bot.
//!
//! This module contains all hardcoded configuration values that define
//! the bot's security boundaries and operational parameters.

/// The Discord user ID of the bot owner.
///
/// This is the single source of truth for owner authority across the entire bot.
/// All owner-gated operations (e.g., /update, commission management, ticket management)
/// must check against this constant.
///
/// **Security**: This value defines the trust boundary. Only this user can perform
/// privileged operations. Changing this value requires a code change and redeploy.
pub const OWNER_ID: u64 = 670_362_326_746_267_678;

/// The GitHub repository for official releases.
///
/// Format: `{owner}/{repo}`
/// Used by the self-updater to fetch releases.
pub const GITHUB_REPO: &str = "Lorian-Workspace/Lorian-s-DiscordBot";

/// The target triple for the current platform.
///
/// Used by the self-updater to select the correct release asset.
pub const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";

/// The base name for release assets.
///
/// The updater looks for: `{ASSET_BASE_NAME}-{TARGET_TRIPLE}`
/// And the checksum file: `{ASSET_BASE_NAME}-{TARGET_TRIPLE}.sha256`
pub const ASSET_BASE_NAME: &str = "lorian-discord-bot";
