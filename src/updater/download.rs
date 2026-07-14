//! Download and verify binary with checksum

use reqwest::redirect::Policy;
use reqwest::Client;
use serenity::futures::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

use super::UpdaterError;

/// Maximum download size (100 MB)
const MAX_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum checksum file size (1 KiB)
const MAX_CHECKSUM_SIZE: u64 = 1024;

/// Allowed redirect hosts
const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// Per-hop redirect policy —- validate each redirect URL against allowed hosts
fn redirect_policy() -> Policy {
    Policy::custom(move |attempt| {
        let url = attempt.url();
        if !url.as_str().starts_with("https://") {
            eprintln!("Redirect rejected: non-HTTPS ({})", url);
            return attempt.error("non-HTTPS redirect");
        }
        let allowed = ALLOWED_HOSTS
            .iter()
            .any(|h| url.as_str().contains(&format!("{}/", h)) || url.host_str() == Some(h));
        if allowed {
            attempt.follow()
        } else {
            eprintln!("Redirect rejected: {} not in allowed hosts", url);
            attempt.error("redirect to disallowed host")
        }
    })
}

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
        .redirect(redirect_policy())
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
    // Must be HTTPS
    if !url.starts_with("https://") {
        return Err(UpdaterError::Network("URL must be HTTPS".to_string()));
    }

    // Must be from GitHub or allowed CDN
    let mut is_allowed = false;
    for host in ALLOWED_HOSTS {
        if url.contains(&format!("{}/", host)) {
            is_allowed = true;
            break;
        }
    }

    if !is_allowed {
        return Err(UpdaterError::Network(
            "URL must be from github.com or allowed CDN".to_string(),
        ));
    }

    // Must be from the correct repo
    if !url.contains(&format!("/{}/", crate::config::GITHUB_REPO)) {
        return Err(UpdaterError::Network(
            "URL must be from correct repository".to_string(),
        ));
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

    // Validate final URL host
    let final_url = response.url().as_str();
    validate_final_url(final_url)?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(UpdaterError::NoReleaseAvailable);
        }
        return Err(UpdaterError::Network(format!("HTTP {}", response.status())));
    }

    let content_length = response.content_length().unwrap_or(0);
    if content_length > MAX_CHECKSUM_SIZE {
        return Err(UpdaterError::Network("Checksum file too large".to_string()));
    }

    // Stream the checksum file with a hard 1 KiB cap — no full body buffering
    let mut buf: Vec<u8> = Vec::with_capacity(MAX_CHECKSUM_SIZE as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream
        .next()
        .await
        .transpose()
        .map_err(|e| UpdaterError::Network(e.to_string()))?
    {
        if buf.len() + chunk.len() > MAX_CHECKSUM_SIZE as usize {
            return Err(UpdaterError::Network("Checksum file too large".to_string()));
        }
        buf.extend_from_slice(&chunk);
    }

    let text = match std::str::from_utf8(&buf) {
        Ok(s) => s,
        Err(_) => {
            return Err(UpdaterError::InvalidResponse(
                "Checksum file is not valid UTF-8".to_string(),
            ));
        }
    };
    // First token must be the 64-char hex SHA-256; second (if any) must be the
    // exact asset filename, so a crafted checksum line cannot redirect us to a
    // different file.
    let expected_asset =
        crate::config::ASSET_BASE_NAME.to_string() + "-" + crate::config::TARGET_TRIPLE;
    let mut iter = text.split_whitespace();
    let checksum = iter
        .next()
        .ok_or_else(|| UpdaterError::InvalidResponse("Empty checksum file".to_string()))?
        .to_lowercase();
    if let Some(name) = iter.next() {
        if name != expected_asset {
            return Err(UpdaterError::InvalidResponse(format!(
                "Checksum filename mismatch: expected {} got {}",
                expected_asset, name
            )));
        }
    }

    // Validate checksum format (64 hex chars for SHA256)
    if checksum.len() != 64 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(UpdaterError::InvalidResponse(
            "Invalid checksum format".to_string(),
        ));
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

    // Validate final URL host
    let final_url = response.url().as_str();
    validate_final_url(final_url)?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(UpdaterError::NoReleaseAvailable);
        }
        return Err(UpdaterError::Network(format!("HTTP {}", response.status())));
    }

    // Check content length
    if let Some(size) = response.content_length() {
        if size > MAX_SIZE {
            return Err(UpdaterError::Network("Binary too large".to_string()));
        }
    }

    // Create file with create_new and 0600 permissions
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output_path)
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?;

    let mut response = response;
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();

    // Stream chunks to bound memory usage
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|e| UpdaterError::Network(e.to_string()))?;

        match chunk {
            Some(bytes) => {
                downloaded += bytes.len() as u64;
                if downloaded > MAX_SIZE {
                    drop(file);
                    let _ = std::fs::remove_file(output_path);
                    return Err(UpdaterError::Network("Binary too large".to_string()));
                }
                hasher.update(&bytes);
                file.write_all(&bytes)
                    .map_err(|e| UpdaterError::FileIo(e.to_string()))?;
            }
            None => break, // stream complete
        }
    }

    // Sync to disk
    file.sync_all()
        .map_err(|e| UpdaterError::FileIo(e.to_string()))?;
    drop(file);

    // Also sync parent directory
    if let Some(parent) = output_path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Validate final URL after redirects
fn validate_final_url(url: &str) -> Result<(), UpdaterError> {
    // Must be HTTPS
    if !url.starts_with("https://") {
        return Err(UpdaterError::Network("Final URL must be HTTPS".to_string()));
    }

    // Must be from allowed hosts
    let mut is_allowed = false;
    for host in ALLOWED_HOSTS {
        if url.contains(&format!("{}/", host)) {
            is_allowed = true;
            break;
        }
    }

    if !is_allowed {
        return Err(UpdaterError::Network(
            "Final URL must be from allowed hosts".to_string(),
        ));
    }

    Ok(())
}
