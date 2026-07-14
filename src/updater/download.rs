//! Download and verify binary with checksum

use reqwest::Client;
use sha2::{Sha256, Digest};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use super::UpdaterError;

/// Maximum download size (100 MB)
const MAX_SIZE: u64 = 100 * 1024 * 1024;

/// Download binary and verify checksum
pub async fn download_and_verify(
    asset_url: &str,
    checksum_url: &str,
    output_path: &Path,
) -> Result<(), UpdaterError> {
    // Validate URLs
    validate_url(asset_url)?;
    validate_url(checksum_url)?;

    let client = Client::builder()
        .user_agent(format!("lorian-discord-bot/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    // Download checksum first
    let expected_checksum = download_checksum(&client, checksum_url).await?;

    // Download binary
    let actual_checksum = download_binary(&client, asset_url, output_path).await?;

    // Verify checksum
    if expected_checksum != actual_checksum {
        // Clean up partial download
        let _ = std::fs::remove_file(output_path);
        return Err(UpdaterError::ChecksumMismatch {
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }

    Ok(())
}

/// Validate URL is from GitHub releases
fn validate_url(url: &str) -> Result<(), UpdaterError> {
    let parsed = url::Url::parse(url).map_err(|e| UpdaterError::Network(e.to_string()))?;

    // Must be HTTPS
    if parsed.scheme() != "https" {
        return Err(UpdaterError::Network("URL must be HTTPS".to_string()));
    }

    // Must be from GitHub
    if parsed.host_str() != Some("github.com") && parsed.host_str() != Some("api.github.com") {
        return Err(UpdaterError::Network("URL must be from github.com".to_string()));
    }

    // Must be from the correct repo
    let path = parsed.path();
    if !path.contains(&format!("/{}/", crate::config::GITHUB_REPO)) {
        return Err(UpdaterError::Network("URL must be from correct repository".to_string()));
    }

    Ok(())
}

/// Download checksum file and parse it
async fn download_checksum(client: &Client, url: &str) -> Result<String, UpdaterError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!("HTTP {}", response.status())));
    }

    let content_length = response.content_length().unwrap_or(0);
    if content_length > 1024 {
        return Err(UpdaterError::Network("Checksum file too large".to_string()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    // Parse checksum from format: "hash  filename" or "hash filename"
    let checksum = text
        .split_whitespace()
        .next()
        .ok_or_else(|| UpdaterError::InvalidResponse("Empty checksum file".to_string()))?
        .to_lowercase();

    // Validate checksum format (64 hex chars for SHA256)
    if checksum.len() != 64 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(UpdaterError::InvalidResponse("Invalid checksum format".to_string()));
    }

    Ok(checksum)
}

/// Download binary and compute checksum
async fn download_binary(
    client: &Client,
    url: &str,
    output_path: &Path,
) -> Result<String, UpdaterError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!("HTTP {}", response.status())));
    }

    // Check content length
    if let Some(size) = response.content_length() {
        if size > MAX_SIZE {
            return Err(UpdaterError::Network("Binary too large".to_string()));
        }
    }

    // Download with streaming to compute checksum
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::create(output_path)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| UpdaterError::Network(e.to_string()))?;
        downloaded += chunk.len() as u64;

        if downloaded > MAX_SIZE {
            drop(file);
            let _ = std::fs::remove_file(output_path);
            return Err(UpdaterError::Network("Binary too large".to_string()));
        }

        hasher.update(&chunk);
        file.write_all(&chunk)
            .map_err(|e| UpdaterError::FileIo(e.to_string()))?;
    }

    // Sync to disk
    file.sync_all().map_err(|e| UpdaterError::FileIo(e.to_string()))?;
    drop(file);

    // Also sync parent directory
    if let Some(parent) = output_path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}
