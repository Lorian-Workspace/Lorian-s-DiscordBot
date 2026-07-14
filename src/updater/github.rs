//! GitHub API client for fetching releases

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use super::{config, UpdaterError};

/// GitHub release information
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

/// GitHub release asset
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Fetch the latest stable release from GitHub
pub async fn fetch_latest_release() -> Result<Release, UpdaterError> {
    let client = Client::builder()
        .user_agent(format!("lorian-discord-bot/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        config::GITHUB_REPO
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(UpdaterError::NoReleaseAvailable);
        }
        return Err(UpdaterError::GitHubApi(format!(
            "HTTP {}",
            response.status()
        )));
    }

    let release: Release = response
        .json()
        .await
        .map_err(|e| UpdaterError::InvalidResponse(e.to_string()))?;

    // Reject drafts and prereleases
    if release.draft || release.prerelease {
        return Err(UpdaterError::NoReleaseAvailable);
    }

    // Verify assets exist
    let expected_asset = format!("{}-{}", config::ASSET_BASE_NAME, config::TARGET_TRIPLE);
    let has_asset = release.assets.iter().any(|a| a.name == expected_asset);
    let has_checksum = release
        .assets
        .iter()
        .any(|a| a.name == format!("{}.sha256", expected_asset));

    if !has_asset || !has_checksum {
        return Err(UpdaterError::NoReleaseAvailable);
    }

    Ok(release)
}
