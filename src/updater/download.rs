//! Download and verify binary with checksum.

use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use serenity::futures::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::UpdaterError;

/// Maximum download size (100 MiB).
const MAX_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum checksum file size (1 KiB).
const MAX_CHECKSUM_SIZE: u64 = 1024;

/// Allowed redirect/final hosts.
const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

fn expected_asset_name() -> String {
    format!(
        "{}-{}",
        crate::config::ASSET_BASE_NAME,
        crate::config::TARGET_TRIPLE
    )
}

fn parse_https_url(url: &str, subject: &str) -> Result<Url, UpdaterError> {
    let parsed =
        Url::parse(url).map_err(|e| UpdaterError::Network(format!("{subject} is invalid: {e}")))?;

    if parsed.scheme() != "https" {
        return Err(UpdaterError::Network(format!("{subject} must use HTTPS")));
    }

    Ok(parsed)
}

/// Validate the initial GitHub release download URLs.
fn validate_release_download_url(url: &str) -> Result<(), UpdaterError> {
    let parsed = parse_https_url(url, "URL")?;

    if parsed.host_str() != Some("github.com") {
        return Err(UpdaterError::Network(
            "URL host must be exactly github.com".to_string(),
        ));
    }

    let expected_prefix = format!("/{}/releases/download/", crate::config::GITHUB_REPO);
    if !parsed.path().starts_with(&expected_prefix) {
        return Err(UpdaterError::Network(
            "URL must be from the configured repository release path".to_string(),
        ));
    }

    Ok(())
}

/// Validate each redirect hop and the final response URL.
fn validate_redirect_target(url: &Url) -> Result<(), UpdaterError> {
    if url.scheme() != "https" {
        return Err(UpdaterError::Network(
            "redirect/final URL must use HTTPS".to_string(),
        ));
    }

    let host = url.host_str().ok_or_else(|| {
        UpdaterError::Network("redirect/final URL must include a host".to_string())
    })?;

    if ALLOWED_HOSTS.contains(&host) {
        Ok(())
    } else {
        Err(UpdaterError::Network(format!(
            "redirect/final URL host {host} is not allowed"
        )))
    }
}

/// Per-hop redirect policy: validate each redirect URL against allowed hosts.
fn redirect_policy() -> Policy {
    Policy::custom(
        move |attempt| match validate_redirect_target(attempt.url()) {
            Ok(()) => attempt.follow(),
            Err(err) => {
                eprintln!("Redirect rejected ({}): {}", attempt.url(), err);
                attempt.error(err.to_string())
            }
        },
    )
}

/// Validate final URL after redirects.
fn validate_final_url(url: &str) -> Result<(), UpdaterError> {
    let parsed = parse_https_url(url, "Final URL")?;
    validate_redirect_target(&parsed)
}

#[derive(Debug)]
struct ChecksumAccumulator {
    buf: Vec<u8>,
    max_size: usize,
}

impl ChecksumAccumulator {
    fn new(max_size: u64) -> Self {
        Self {
            buf: Vec::with_capacity(max_size as usize),
            max_size: max_size as usize,
        }
    }

    fn push_chunk(&mut self, chunk: &[u8]) -> Result<(), UpdaterError> {
        if self.buf.len().saturating_add(chunk.len()) > self.max_size {
            return Err(UpdaterError::Network("Checksum file too large".to_string()));
        }

        self.buf.extend_from_slice(chunk);
        Ok(())
    }

    fn finish(self, expected_asset: &str) -> Result<String, UpdaterError> {
        let text = std::str::from_utf8(&self.buf).map_err(|_| {
            UpdaterError::InvalidResponse("Checksum file is not valid UTF-8".to_string())
        })?;

        parse_checksum_text(text, expected_asset)
    }
}

fn parse_checksum_text(text: &str, expected_asset: &str) -> Result<String, UpdaterError> {
    let mut iter = text.split_whitespace();

    let checksum = iter
        .next()
        .ok_or_else(|| UpdaterError::InvalidResponse("Empty checksum file".to_string()))?
        .to_ascii_lowercase();

    let filename = iter.next().ok_or_else(|| {
        UpdaterError::InvalidResponse("Checksum file must include the asset filename".to_string())
    })?;

    if filename != expected_asset {
        return Err(UpdaterError::InvalidResponse(format!(
            "Checksum filename mismatch: expected {expected_asset} got {filename}"
        )));
    }

    if iter.next().is_some() {
        return Err(UpdaterError::InvalidResponse(
            "Checksum file has unexpected trailing tokens".to_string(),
        ));
    }

    if checksum.len() != 64 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(UpdaterError::InvalidResponse(
            "Invalid checksum format".to_string(),
        ));
    }

    Ok(checksum)
}

#[derive(Debug)]
struct BinaryDownloadWriter {
    file: std::fs::File,
    output_path: PathBuf,
    downloaded: u64,
    max_size: u64,
    hasher: Sha256,
}

impl BinaryDownloadWriter {
    fn new(output_path: &Path, max_size: u64) -> Result<Self, UpdaterError> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(output_path)
            .map_err(|e| UpdaterError::FileIo(e.to_string()))?;

        Ok(Self {
            file,
            output_path: output_path.to_path_buf(),
            downloaded: 0,
            max_size,
            hasher: Sha256::new(),
        })
    }

    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), UpdaterError> {
        let next_size = self
            .downloaded
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| UpdaterError::Network("Binary too large".to_string()))?;

        if next_size > self.max_size {
            return Err(UpdaterError::Network("Binary too large".to_string()));
        }

        self.downloaded = next_size;
        self.hasher.update(bytes);
        self.file
            .write_all(bytes)
            .map_err(|e| UpdaterError::FileIo(e.to_string()))
    }

    fn cleanup(&mut self) {
        let _ = std::fs::remove_file(&self.output_path);
    }

    fn finish(self) -> Result<String, UpdaterError> {
        self.file
            .sync_all()
            .map_err(|e| UpdaterError::FileIo(e.to_string()))?;
        drop(self.file);

        if let Some(parent) = self.output_path.parent() {
            if let Ok(dir) = std::fs::File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        Ok(format!("{:x}", self.hasher.finalize()))
    }
}

/// Download binary and verify checksum.
pub async fn download_and_verify(
    asset_url: &str,
    checksum_url: &str,
    output_path: &Path,
) -> Result<(), UpdaterError> {
    validate_release_download_url(asset_url)?;
    validate_release_download_url(checksum_url)?;

    let client = Client::builder()
        .user_agent(format!("lorian-discord-bot/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .redirect(redirect_policy())
        .build()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    let expected_checksum = download_checksum(&client, checksum_url).await?;
    let actual_checksum = download_binary(&client, asset_url, output_path).await?;

    if expected_checksum != actual_checksum {
        let _ = std::fs::remove_file(output_path);
        return Err(UpdaterError::ChecksumMismatch {
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }

    Ok(())
}

/// Download checksum file and parse it.
async fn download_checksum(client: &Client, url: &str) -> Result<String, UpdaterError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    validate_final_url(response.url().as_str())?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(UpdaterError::NoReleaseAvailable);
        }
        return Err(UpdaterError::Network(format!("HTTP {}", response.status())));
    }

    if response.content_length().unwrap_or(0) > MAX_CHECKSUM_SIZE {
        return Err(UpdaterError::Network("Checksum file too large".to_string()));
    }

    let mut accumulator = ChecksumAccumulator::new(MAX_CHECKSUM_SIZE);
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream
        .next()
        .await
        .transpose()
        .map_err(|e| UpdaterError::Network(e.to_string()))?
    {
        accumulator.push_chunk(&chunk)?;
    }

    accumulator.finish(&expected_asset_name())
}

/// Download binary and compute checksum.
async fn download_binary(
    client: &Client,
    url: &str,
    output_path: &Path,
) -> Result<String, UpdaterError> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdaterError::Network(e.to_string()))?;

    validate_final_url(response.url().as_str())?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            return Err(UpdaterError::NoReleaseAvailable);
        }
        return Err(UpdaterError::Network(format!("HTTP {}", response.status())));
    }

    if let Some(size) = response.content_length() {
        if size > MAX_SIZE {
            return Err(UpdaterError::Network("Binary too large".to_string()));
        }
    }

    let mut writer = BinaryDownloadWriter::new(output_path, MAX_SIZE)?;

    loop {
        match response.chunk().await {
            Ok(Some(bytes)) => {
                if let Err(err) = writer.write_chunk(&bytes) {
                    writer.cleanup();
                    return Err(err);
                }
            }
            Ok(None) => return writer.finish(),
            Err(err) => {
                writer.cleanup();
                return Err(UpdaterError::Network(err.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn validate_release_download_url_rejects_spoofed_host() {
        let url = format!(
            "https://github.com.evil.example/{}/releases/download/v1.2.3/{}",
            crate::config::GITHUB_REPO,
            expected_asset_name()
        );

        assert!(validate_release_download_url(&url).is_err());
    }

    #[test]
    fn validate_release_download_url_rejects_wrong_repo() {
        let url = format!(
            "https://github.com/not-the-repo/releases/download/v1.2.3/{}",
            expected_asset_name()
        );

        assert!(validate_release_download_url(&url).is_err());
    }

    #[test]
    fn validate_redirect_target_enforces_https_and_exact_host() {
        let github = Url::parse("https://github.com/example").unwrap();
        let cdn = Url::parse("https://release-assets.githubusercontent.com/asset").unwrap();
        let spoofed = Url::parse("https://github.com.evil.example/example").unwrap();
        let http = Url::parse("http://github.com/example").unwrap();

        assert!(validate_redirect_target(&github).is_ok());
        assert!(validate_redirect_target(&cdn).is_ok());
        assert!(validate_redirect_target(&spoofed).is_err());
        assert!(validate_redirect_target(&http).is_err());
    }

    #[test]
    fn checksum_accumulator_rejects_mismatched_filename() {
        let mut accumulator = ChecksumAccumulator::new(MAX_CHECKSUM_SIZE);
        let payload = format!("{:064x} wrong-file", 0xabu8);

        accumulator.push_chunk(payload.as_bytes()).unwrap();

        let err = accumulator.finish(&expected_asset_name()).unwrap_err();
        assert!(err.to_string().contains("Checksum filename mismatch"));
    }

    #[test]
    fn checksum_accumulator_rejects_stream_over_limit() {
        let mut accumulator = ChecksumAccumulator::new(MAX_CHECKSUM_SIZE);

        accumulator
            .push_chunk(&vec![b'a'; MAX_CHECKSUM_SIZE as usize])
            .unwrap();

        let err = accumulator.push_chunk(b"b").unwrap_err();
        assert_eq!(err.to_string(), "Network error: Checksum file too large");
    }

    #[test]
    fn binary_download_writer_rejects_stream_over_limit_and_removes_partial() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("download.part");
        let mut writer = BinaryDownloadWriter::new(&output_path, 4).unwrap();

        writer.write_chunk(b"abcd").unwrap();

        let err = writer.write_chunk(b"e").unwrap_err();
        writer.cleanup();

        assert_eq!(err.to_string(), "Network error: Binary too large");
        assert!(!output_path.exists());
    }
}
