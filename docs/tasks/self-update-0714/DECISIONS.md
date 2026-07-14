# Self-Update & Fixed Owner — Decisions

## Scope

### In Scope
1. **Fixed Owner Consolidation**: Single `OWNER_ID: u64 = 670362326746267678` as bot-wide authority
2. **Self-Updater (方案C)**: Linux x86_64 self-contained auto-update from GitHub releases
3. **Release Workflow**: GitHub Actions on `v*` tags with checksum verification
4. **`/update` Command**: Owner-only manual trigger with ephemeral feedback

### Out of Scope
- Multi-architecture support (Linux x86_64 only)
- Runtime GitHub token or arbitrary URL support
- Automatic rollback after new binary exec/crash
- Multi-guild or multi-owner support
- Windows/macOS updater paths
- Rollback UI or command

## Key Decisions

### Owner Authority
**Decision**: Single hardcoded `OWNER_ID` constant in `src/config.rs`  
**Rationale**: 
- Eliminates env var drift (`DISCORD_OWNER_ID`, `OWNER_DISCORD_ID`)
- Removes hardcoded magic numbers scattered across codebase
- Single source of truth for all owner-gated operations
- No runtime configuration needed for security-critical authority

**Tradeoff**: Requires code change + redeploy to change owner  
**Mitigation**: Owner changes are rare; redeploy is acceptable for security boundary change

### Self-Updater Architecture (方案C)
**Decision**: GitHub releases only, no runtime token, checksum verification, atomic replace + exec restart  
**Rationale**:
- Trust root = GitHub repo + workflow + TLS + checksum (not publisher signature)
- No secrets in binary or env
- Minimal attack surface: fixed repo, fixed asset pattern, HTTPS only
- Atomic operations prevent corruption

**Tradeoff**: Linux x86_64 only, no Windows/macOS  
**Mitigation**: Target platform for this deployment; other platforms can be added later

**Tradeoff**: No automatic rollback after exec  
**Rationale**: If new binary crashes, manual `.bak` restore required; auto-rollback adds complexity and race conditions

**Tradeoff**: Checksum is not publisher signature  
**Rationale**: GitHub workflow generates both binary and checksum; compromise of workflow compromises both; trust root is GitHub account security + 2FA

### Update Timing
**Decision**: 
- Release builds: startup delay + 6h interval auto-check
- Debug builds: no auto-update, manual `/update` only
- `AUTO_UPDATE_ENABLED=false` kill switch

**Rationale**:
- Prevents update storms on startup
- 6h interval balances freshness vs API rate limits
- Debug builds shouldn't auto-update during development
- Kill switch for emergency disable

**Tradeoff**: Bot may run old version for up to 6h  
**Mitigation**: Manual `/update` available for immediate update

### Concurrency Control
**Decision**: Tokio `try_lock` + cross-process std file advisory `try_lock`  
**Rationale**:
- Prevents concurrent updates within same process
- Prevents concurrent updates across processes (e.g., multiple bot instances)
- `try_lock` is non-blocking; busy = no-op

**Tradeoff**: If lock held, update skipped entirely  
**Mitigation**: Lock holder will complete update; next check will succeed

### File Operations
**Decision**: 
- Download to `.part` file in same directory
- SHA256 verify before rename
- Staged binary `--self-check` / `--version` with exact version match
- File + parent fsync
- Keep one `.bak` file
- Same-FS atomic rename
- `exec(2)` same PID with original argv
- On exec failure: atomic restore + continue old process

**Rationale**:
- `.part` file prevents partial downloads from being used
- SHA256 verify ensures integrity
- Self-check catches incompatible binaries before replace
- fsync ensures durability across crash
- `.bak` enables manual rollback
- Atomic rename prevents corruption
- exec preserves PID (systemd/supervisor friendly)
- Restore on exec failure ensures continuity

**Tradeoff**: Complex error handling  
**Mitigation**: All paths tested; no-panic guarantee; fail-safe defaults

### Discord UX
**Decision**: 
- Immediate ephemeral ACK
- Unauthorized = ephemeral deny, no side effect
- Clear status messages: up-to-date / no release / busy / error
- All `/update` actions owner-only

**Rationale**:
- Ephemeral messages don't clutter channel
- Immediate ACK prevents Discord timeout
- Clear messages help owner diagnose issues
- Owner-only prevents unauthorized update attempts

**Tradeoff**: Owner must check ephemeral message for result  
**Mitigation**: Clear, actionable messages

### Release Workflow
**Decision**:
- Track `Cargo.lock` (remove from `.gitignore`)
- Single workflow on `v*` tags
- Tag must match `Cargo.toml` version exactly
- Pinned action SHAs (40-char)
- Minimal permissions: `contents: write` only
- `cargo test --locked` before build
- Generate raw asset + `.sha256`
- Fail on mismatch

**Rationale**:
- `Cargo.lock` ensures reproducible builds
- Pinned SHAs prevent supply-chain attacks
- Minimal permissions reduce blast radius
- Test before build catches issues early
- Checksum verification ensures integrity

**Tradeoff**: Must manually update action SHAs for security patches  
**Mitigation**: Dependabot or manual monitoring

## Dependencies

### Added
- `semver`: Version parsing and comparison
- `sha2`: SHA256 checksum verification

### Reused
- `reqwest`: HTTP client (already in project)
- `tokio`: Async runtime (already in project)
- `serde`/`serde_json`: JSON parsing (already in project)

### Not Added
- `self_update`: Too heavy, opaque internals
- `cargo-dist`: Overkill for single-binary update
- File lock crates: std `File::try_lock` sufficient

## Security Considerations

### Threat Model
1. **GitHub account compromise**: Attacker pushes malicious release
   - Mitigation: 2FA on owner account, checksum verification
   - Residual risk: If account compromised, attacker can update checksum too

2. **Network MITM**: Attacker intercepts download
   - Mitigation: HTTPS only, GitHub host allowlist
   - Residual risk: None if TLS properly validated

3. **Local file tampering**: Attacker modifies binary on disk
   - Mitigation: Checksum verification before exec
   - Residual risk: None if checksum file not tampered

4. **Race condition**: Multiple update attempts
   - Mitigation: Cross-process file lock
   - Residual risk: None if lock works correctly

5. **Crash during update**: Power loss or OOM
   - Mitigation: Atomic rename, `.bak` file, pending record
   - Residual risk: Manual `.bak` restore required

### Not Protected Against
- GitHub workflow compromise (trust root)
- Owner account compromise (trust root)
- Kernel-level file system corruption (out of scope)

## Testing Strategy

### Unit Tests
- Owner gate logic
- SemVer parsing and comparison
- Checksum parsing and verification
- URL asset allowlist
- Recovery state machine

### Integration Tests
- None (no live Discord/binary replacement)

### Manual Verification
- `cargo check --locked`
- `cargo test --locked`
- `cargo clippy --all-targets --all-features --locked`
- `rustfmt --edition 2021 --check` on changed files
- `rg` for old owner strings
- Inspect workflow YAML

## Rollout Plan

1. Merge to main
2. Create `v0.2.0` tag (or next version)
3. Workflow builds release + checksum
4. Bot auto-updates on next 6h check (or manual `/update`)
5. Monitor logs for update success/failure

## Future Work

- Multi-architecture support
- Rollback command (manual `.bak` restore)
- Update notification in channel (not just ephemeral)
- Metrics/telemetry for update success rate
- Staged rollout (canary releases)
