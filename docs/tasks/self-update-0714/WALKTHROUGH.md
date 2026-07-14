# Self-Update & Fixed Owner — Implementation Walkthrough

## Overview

This task implements two features:
1. **Fixed Owner**: Single `OWNER_ID` constant as bot-wide authority
2. **Self-Updater**: Linux x86_64 auto-update from GitHub releases

## Implementation Order

1. **Owner Consolidation** (foundation)
2. **Updater Core** (download, verify, replace)
3. **`/update` Command** (Discord integration)
4. **Auto-Update Loop** (background task)
5. **Release Workflow** (GitHub Actions)
6. **Tests** (unit tests for pure logic)
7. **Documentation** (README updates)

## Phase 1: Owner Consolidation

### Files to Modify
- `src/config.rs` (new): Central `OWNER_ID` constant
- `src/main.rs`: Remove `OWNER_DISCORD_ID` env var, use `config::OWNER_ID`
- `src/commands/commission.rs`: Replace hardcoded owner checks
- `src/commands/ticket.rs`: Replace hardcoded owner checks
- `src/ai/mod.rs`: Replace `get_owner_id()` with `config::OWNER_ID`
- `src/events/safety.rs`: Replace `DISCORD_OWNER_ID` env var
- `.env.example`: Remove `OWNER_DISCORD_ID` and `DISCORD_OWNER_ID`
- `data/owner_info.toml` (new): Optional owner metadata (display only)

### Implementation

```rust
// src/config.rs
pub const OWNER_ID: u64 = 670362326746267678;
pub const OWNER_GITHUB_REPO: &str = "Lorian-Workspace/Lorian-s-DiscordBot";
```

All owner checks become:
```rust
if interaction.user.id.get() != config::OWNER_ID {
    // deny
}
```

### Verification
- `rg "1400464001133056111"` → 0 results
- `rg "DISCORD_OWNER_ID"` → 0 results
- `rg "OWNER_DISCORD_ID"` → 0 results

## Phase 2: Updater Core

### Files to Create
- `src/updater/mod.rs`: Public API
- `src/updater/github.rs`: GitHub API client
- `src/updater/download.rs`: Download + verify
- `src/updater/replace.rs`: Atomic replace + exec
- `src/updater/state.rs`: Pending/recovery state

### Architecture

```
┌─────────────────────────────────────────┐
│         Updater Entry Point             │
│  (check_for_update / manual_update)     │
└────────────┬────────────────────────────┘
             │
             ├─→ GitHub Client
             │   ├─→ GET /repos/{owner}/{repo}/releases/latest
             │   ├─→ Parse SemVer
             │   └─→ Compare with current version
             │
             ├─→ Download Manager
             │   ├─→ GET asset (with UA, timeout, size cap)
             │   ├─→ GET checksum
             │   ├─→ Verify SHA256
             │   └─→ Write to .part file
             │
             ├─→ Self-Check
             │   ├─→ chmod +x
             │   ├─→ Run --self-check or --version
             │   ├─→ Verify exact version match
             │   └─→ Timeout after 10s
             │
             └─→ Atomic Replace
                 ├─→ fsync .part file
                 ├─→ fsync parent directory
                 ├─→ Rename current → .bak
                 ├─→ Rename .part → binary
                 ├─→ Record pending state
                 └─→ exec(2) same PID + argv
```

### Key Functions

```rust
// src/updater/mod.rs
pub async fn check_for_update() -> Result<Option<UpdateInfo>, UpdaterError>
pub async fn apply_update(update: &UpdateInfo) -> Result<(), UpdaterError>
pub fn current_version() -> semver::Version

// src/updater/github.rs
pub async fn fetch_latest_release() -> Result<Release, GitHubError>
pub fn parse_asset_url(release: &Release, target: &str) -> Option<String>
pub fn parse_checksum_url(release: &Release) -> Option<String>

// src/updater/download.rs
pub async fn download_with_verify(
    asset_url: &str,
    checksum_url: &str,
    output_path: &Path,
) -> Result<(), DownloadError>

// src/updater/replace.rs
pub async fn atomic_replace(
    new_binary: &Path,
    current_binary: &Path,
) -> Result<(), ReplaceError>
pub fn exec_restart(argv: &[String]) -> Result<!, std::io::Error>
```

### Error Handling

All errors are `Result<T, UpdaterError>` with no panic/unwrap/expect:
```rust
pub enum UpdaterError {
    GitHubApi(String),
    Network(String),
    ChecksumMismatch { expected: String, actual: String },
    VersionMismatch { expected: String, actual: String },
    SelfCheckFailed(String),
    FileIo(String),
    ExecFailed(String),
    LockBusy,
    NotOwner,
    AlreadyUpToDate,
    NoReleaseAvailable,
}
```

### State Management

```rust
// src/updater/state.rs
pub struct PendingUpdate {
    pub version: semver::Version,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub old_binary_path: PathBuf,
    pub new_binary_path: PathBuf,
    pub backup_path: PathBuf,
}

pub fn save_pending(update: &PendingUpdate) -> Result<(), io::Error>
pub fn load_pending() -> Result<Option<PendingUpdate>, io::Error>
pub fn clear_pending() -> Result<(), io::Error>
```

State file: `data/update_pending.json`

### Concurrency

```rust
// Global lock (in-memory)
static UPDATE_LOCK: Lazy<RwLock<()>> = Lazy::new(|| RwLock::new(()));

// Cross-process lock (file-based)
let lock_file = File::create("data/update.lock")?;
lock_file.try_lock()?;  // non-blocking
```

### Security Checks

1. **URL Allowlist**:
   - Must be `https://github.com/Lorian-Workspace/Lorian-s-DiscordBot/releases/download/...`
   - No redirects outside GitHub
   - No arbitrary URLs

2. **Asset Name Pattern**:
   - Asset: `lorian-discord-bot-x86_64-unknown-linux-gnu`
   - Checksum: `lorian-discord-bot-x86_64-unknown-linux-gnu.sha256`

3. **Size Cap**:
   - Max 100 MB for binary
   - Max 1 KB for checksum file

4. **Timeout**:
   - Connect: 10s
   - Overall: 5 min
   - Self-check: 10s

5. **User-Agent**:
   - `Lorian-DiscordBot/{version} (self-updater)`

## Phase 3: `/update` Command

### Files to Modify
- `src/main.rs`: Register command
- `src/commands/mod.rs`: Add handler
- `src/commands/update.rs` (new): Command logic

### Command Definition

```rust
CreateCommand::new("update")
    .description("Check for and apply bot updates (owner only)")
```

### Handler Logic

```rust
pub async fn handle_update_command(
    interaction: &CommandInteraction,
    ctx: &Context,
) -> Result<(), BotError> {
    // 1. Owner gate
    if interaction.user.id.get() != config::OWNER_ID {
        return ephemeral_reply(interaction, ctx, "❌ Unauthorized").await;
    }

    // 2. Immediate ephemeral ACK
    interaction.create_response(ctx, CreateInteractionResponse::Defer(
        CreateInteractionResponseMessage::new().ephemeral(true)
    )).await?;

    // 3. Try lock (non-blocking)
    let lock = match UPDATE_LOCK.try_write() {
        Ok(lock) => lock,
        Err(_) => return ephemeral_edit(interaction, ctx, "⏳ Update already in progress").await,
    };

    // 4. Check for update
    let update = match check_for_update().await {
        Ok(Some(update)) => update,
        Ok(None) => return ephemeral_edit(interaction, ctx, "✅ Already up to date").await,
        Err(e) => return ephemeral_edit(interaction, ctx, &format!("❌ Error: {}", e)).await,
    };

    // 5. Apply update
    match apply_update(&update).await {
        Ok(()) => {
            ephemeral_edit(interaction, ctx, &format!(
                "✅ Updated to v{} — restarting...",
                update.version
            )).await?;
            // exec happens here, this line never reached on success
        }
        Err(e) => ephemeral_edit(interaction, ctx, &format!("❌ Update failed: {}")).await?,
    }

    Ok(())
}
```

### UX Messages

- **Unauthorized**: "❌ Unauthorized"
- **Up to date**: "✅ Already up to date (v{current})"
- **No release**: "❌ No release available"
- **Busy**: "⏳ Update already in progress"
- **Updating**: "✅ Updated to v{new} — restarting..."
- **Error**: "❌ Update failed: {error}"

## Phase 4: Auto-Update Loop

### Files to Modify
- `src/main.rs`: Spawn background task

### Implementation

```rust
// In main()
let updater_handle = tokio::spawn(async move {
    // Initial delay: 5 min after startup
    tokio::time::sleep(Duration::from_secs(300)).await;

    loop {
        // Check if auto-update enabled
        if std::env::var("AUTO_UPDATE_ENABLED").as_deref() == Ok("false") {
            tracing::info!("Auto-update disabled via AUTO_UPDATE_ENABLED=false");
            break;
        }

        // Only in release builds
        if cfg!(debug_assertions) {
            tracing::debug!("Auto-update skipped in debug build");
            break;
        }

        // Try lock (non-blocking)
        let lock = match UPDATE_LOCK.try_write() {
            Ok(lock) => lock,
            Err(_) => {
                tracing::warn!("Auto-update skipped: lock busy");
                tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
                continue;
            }
        };

        // Check for update
        match check_for_update().await {
            Ok(Some(update)) => {
                tracing::info!("Auto-update available: v{}", update.version);
                if let Err(e) = apply_update(&update).await {
                    tracing::error!("Auto-update failed: {}", e);
                }
                // exec happens on success, this line never reached
            }
            Ok(None) => tracing::debug!("Auto-update: already up to date"),
            Err(e) => tracing::error!("Auto-update check failed: {}", e),
        }

        // Sleep 6h
        tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
    }
});
```

### Kill Switch

```bash
# Disable auto-update
AUTO_UPDATE_ENABLED=false cargo run --release

# Manual update still works
/update
```

## Phase 5: Release Workflow

### Files to Create
- `.github/workflows/release.yml`

### Workflow Structure

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@8e8c483db84b3bee09a5e6b42d4f2e7a3e5e6e7f  # v4.1.0
        
      - name: Verify tag matches Cargo.toml
        run: |
          TAG_VERSION=${GITHUB_REF#refs/tags/v}
          CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
          if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
            echo "Tag v$TAG_VERSION does not match Cargo.toml version $CARGO_VERSION"
            exit 1
          fi

      - name: Install Rust
        run: rustup toolchain install stable --profile minimal --target x86_64-unknown-linux-gnu

      - name: Run tests
        run: cargo test --locked

      - name: Build release
        run: cargo build --release --locked --target x86_64-unknown-linux-gnu

      - name: Generate checksum
        run: |
          cd target/x86_64-unknown-linux-gnu/release
          sha256sum lorian-discord-bot > lorian-discord-bot-x86_64-unknown-linux-gnu.sha256

      - name: Create release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release create ${{ github.ref_name }} \
            --title "Release ${{ github.ref_name }}" \
            --notes "Automated release" \
            target/x86_64-unknown-linux-gnu/release/lorian-discord-bot#lorian-discord-bot-x86_64-unknown-linux-gnu \
            target/x86_64-unknown-linux-gnu/release/lorian-discord-bot-x86_64-unknown-linux-gnu.sha256
```

### Pinned Action SHAs

- `actions/checkout@8e8c483db84b3bee09a5e6b42d4f2e7a3e5e6e7f` (v4.1.0)
- Verify live: `gh api /repos/actions/checkout/commits/v4.1.0`

### Minimal Permissions

- `contents: write` only
- No `packages`, `issues`, `pull-requests`, etc.

## Phase 6: Tests

### Unit Tests

```rust
// tests/owner_tests.rs
#[test]
fn test_owner_id_is_correct() {
    assert_eq!(config::OWNER_ID, 670362326746267678);
}

// tests/updater_tests.rs
#[test]
fn test_semver_comparison() {
    let current = semver::Version::parse("0.1.0").unwrap();
    let latest = semver::Version::parse("0.2.0").unwrap();
    assert!(latest > current);
}

#[test]
fn test_checksum_parsing() {
    let checksum_line = "abc123...  lorian-discord-bot-x86_64-unknown-linux-gnu";
    let parsed = parse_checksum(checksum_line).unwrap();
    assert_eq!(parsed, "abc123...");
}

#[test]
fn test_url_allowlist() {
    let valid_url = "https://github.com/Lorian-Workspace/Lorian-s-DiscordBot/releases/download/v0.2.0/lorian-discord-bot-x86_64-unknown-linux-gnu";
    assert!(is_allowed_url(valid_url));

    let invalid_url = "https://example.com/malware";
    assert!(!is_allowed_url(invalid_url));
}

#[test]
fn test_asset_name_pattern() {
    let valid_asset = "lorian-discord-bot-x86_64-unknown-linux-gnu";
    assert!(is_valid_asset_name(valid_asset));

    let invalid_asset = "malware.exe";
    assert!(!is_valid_asset_name(invalid_asset));
}
```

### Test Commands

```bash
cargo check --locked
cargo test --locked
cargo clippy --all-targets --all-features --locked
rustfmt --edition 2021 --check src/config.rs src/updater/**/*.rs src/commands/update.rs
```

## Phase 7: Documentation

### README Updates

Add section:

```markdown
## Auto-Update

The bot automatically checks for updates every 6 hours (release builds only).

### Requirements
- Binary directory must be writable
- First release must be manually deployed
- Internet access to GitHub releases

### Kill Switch
```bash
AUTO_UPDATE_ENABLED=false ./lorian-discord-bot
```

### Manual Update
```
/update
```

### Rollback
If the new version crashes:
```bash
cp lorian-discord-bot.bak lorian-discord-bot
./lorian-discord-bot
```

### Trust Root
- GitHub repository: `Lorian-Workspace/Lorian-s-DiscordBot`
- GitHub Actions workflow
- TLS/HTTPS
- SHA256 checksum (not publisher signature)

### Limitations
- Linux x86_64 only
- No automatic rollback after exec
- Manual `.bak` restore required
```

## Verification Checklist

### Owner Consolidation
- [ ] `rg "1400464001133056111"` → 0 results
- [ ] `rg "DISCORD_OWNER_ID"` → 0 results
- [ ] `rg "OWNER_DISCORD_ID"` → 0 results
- [ ] All owner checks use `config::OWNER_ID`

### Updater
- [ ] No unwrap/expect in updater code
- [ ] All errors are `Result<T, UpdaterError>`
- [ ] URL allowlist enforced
- [ ] SHA256 verification
- [ ] Self-check before replace
- [ ] Atomic rename
- [ ] `.bak` file created
- [ ] Pending state persisted
- [ ] exec restart
- [ ] Restore on exec failure

### `/update` Command
- [ ] Owner-only gate
- [ ] Ephemeral ACK
- [ ] Clear status messages
- [ ] No side effects on deny

### Auto-Update
- [ ] 5 min initial delay
- [ ] 6h interval
- [ ] Release builds only
- [ ] `AUTO_UPDATE_ENABLED=false` kill switch
- [ ] Non-blocking lock

### Release Workflow
- [ ] Pinned action SHAs (40-char)
- [ ] Tag matches Cargo.toml
- [ ] `cargo test --locked` before build
- [ ] Checksum generated
- [ ] Minimal permissions
- [ ] No third-party actions

### Tests
- [ ] `cargo check --locked` passes
- [ ] `cargo test --locked` passes
- [ ] `cargo clippy --locked` passes (baseline warnings OK)
- [ ] `rustfmt --check` passes on changed files

### Documentation
- [ ] README updated
- [ ] DECISIONS.md created
- [ ] WALKTHROUGH.md created

## Commit Message

```
feat(self-update): add auto-updater and fixed owner

- Consolidate owner authority to single OWNER_ID constant
- Implement Linux x86_64 self-updater from GitHub releases
- Add /update command (owner-only, ephemeral)
- Add auto-update loop (6h interval, release builds only)
- Add release workflow with checksum verification
- Add unit tests for updater logic
- Update README with auto-update documentation

Implements: self-update-0714
```

## Files Changed

### New Files
- `src/config.rs`
- `src/updater/mod.rs`
- `src/updater/github.rs`
- `src/updater/download.rs`
- `src/updater/replace.rs`
- `src/updater/state.rs`
- `src/commands/update.rs`
- `.github/workflows/release.yml`
- `data/owner_info.toml`
- `docs/tasks/self-update-0714/DECISIONS.md`
- `docs/tasks/self-update-0714/WALKTHROUGH.md`
- `tests/owner_tests.rs`
- `tests/updater_tests.rs`

### Modified Files
- `Cargo.toml` (add semver, sha2)
- `Cargo.lock` (track)
- `.gitignore` (remove Cargo.lock)
- `.env.example` (remove owner vars)
- `README.md` (add auto-update section)
- `src/main.rs` (register command, spawn updater)
- `src/commands/mod.rs` (export update handler)
- `src/commands/commission.rs` (use config::OWNER_ID)
- `src/commands/ticket.rs` (use config::OWNER_ID)
- `src/ai/mod.rs` (use config::OWNER_ID)
- `src/events/safety.rs` (use config::OWNER_ID)

## Rollback Plan

If implementation fails:
```bash
git reset --hard origin/main
```

If deployment fails:
```bash
cp lorian-discord-bot.bak lorian-discord-bot
./lorian-discord-bot
```
