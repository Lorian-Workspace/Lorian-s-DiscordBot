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

/// The GitHub account whose public activity is announced by the bot.
///
/// Used by the GitHub feed task (`commands::github`) to poll
/// `https://api.github.com/users/{GITHUB_USER}/events/public`.
pub const GITHUB_USER: &str = "Solar2004";

/// Parsed auto-update configuration.
///
/// Behaviour (precedence — first match wins):
/// 1. Unset → enabled (true)
/// 2. Case-insensitive `"true"`, `"yes"`, `"1"` → enabled
/// 3. Case-insensitive `"false"`, `"no"`, `"0"` → disabled
/// 4. Any other value → ERROR (logged at level `error`), disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(not(debug_assertions), test))]
pub struct AutoUpdateConfig {
    pub enabled: bool,
}

#[cfg(any(not(debug_assertions), test))]
impl AutoUpdateConfig {
    /// Parse from an env-var string (e.g. `std::env::var("AUTO_UPDATE_ENABLED")`).
    pub fn from_env(raw: Result<String, std::env::VarError>) -> Self {
        let val = match raw {
            Ok(v) => v,
            Err(std::env::VarError::NotPresent) => return Self { enabled: true },
            Err(std::env::VarError::NotUnicode(_)) => {
                eprintln!(
                    "ERROR: AUTO_UPDATE_ENABLED is not valid Unicode — disabling auto-update."
                );
                return Self { enabled: false };
            }
        };

        match val.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Self { enabled: true },
            "false" | "no" | "0" => Self { enabled: false },
            other => {
                eprintln!(
                    "ERROR: invalid AUTO_UPDATE_ENABLED value {:?} — disabling auto-update. Expected one of: true, false, yes, no, 1, 0.",
                    other
                );
                Self { enabled: false }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_update_default_enabled() {
        let cfg = AutoUpdateConfig::from_env(Err(std::env::VarError::NotPresent));
        assert!(cfg.enabled);
    }

    #[test]
    fn test_auto_update_enabled_values() {
        for val in &["true", "TRUE", "True", "yes", "YES", "1"] {
            let cfg = AutoUpdateConfig::from_env(Ok(val.to_string()));
            assert!(cfg.enabled, "expected enabled for {:?}", val);
        }
    }

    #[test]
    fn test_auto_update_disabled_values() {
        for val in &["false", "FALSE", "False", "no", "NO", "0"] {
            let cfg = AutoUpdateConfig::from_env(Ok(val.to_string()));
            assert!(!cfg.enabled, "expected disabled for {:?}", val);
        }
    }

    #[test]
    fn test_auto_update_invalid_value() {
        // Invalid values log an error and return disabled
        let cfg = AutoUpdateConfig::from_env(Ok("garbage".to_string()));
        assert!(!cfg.enabled);
    }

    #[test]
    fn test_auto_update_empty_value_is_invalid_and_disabled() {
        let cfg = AutoUpdateConfig::from_env(Ok(String::new()));
        assert!(!cfg.enabled);
    }

    #[test]
    fn test_auto_update_not_unicode_is_invalid_and_disabled() {
        #[cfg(unix)]
        {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;

            let cfg = AutoUpdateConfig::from_env(Err(std::env::VarError::NotUnicode(
                OsString::from_vec(vec![0xff]),
            )));
            assert!(!cfg.enabled);
        }
    }
}
