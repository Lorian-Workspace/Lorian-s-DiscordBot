# Self-Update & Fixed Owner — Walkthrough

## What Changed

- `src/config.rs`
  - adds the canonical `OWNER_ID`
  - parses `AUTO_UPDATE_ENABLED` with fail-closed behavior for invalid/non-Unicode values
- `src/commands/update.rs`
  - gates `/update` on `OWNER_ID`
  - returns ephemeral status only to the owner
- `src/main.rs`
  - handles `--version` and `--self-check` before `dotenv` or token access
  - starts background auto-update checks only in release builds
  - avoids a second Discord interaction response on updater-handler errors
- `src/updater/`
  - `github.rs`: fetch latest stable release metadata
  - `download.rs`: exact GitHub release-path validation, redirect/final-host validation, checksum parsing, streamed download limits
  - `replace.rs`: `.bak` rollback on replace failure
  - `state.rs`: O_NOFOLLOW state persistence, atomic save, startup reconcile

## Runtime Flow

1. `/update` or the release-build background task calls `check_for_update()`.
2. The updater fetches `/releases/latest`, rejects drafts/prereleases, and compares strict stable SemVer.
3. The asset URL is fixed to:

```text
https://github.com/Lorian-Workspace/Lorian-s-DiscordBot/releases/download/vX.Y.Z/lorian-discord-bot-x86_64-unknown-linux-gnu
```

4. The checksum is downloaded first, limited to 1 KiB, and must name the exact asset.
5. The binary is streamed to `.part`, limited to 100 MiB, and hashed while writing.
6. The staged binary is made executable and run with `--version`.
7. If the version matches, the updater renames the current binary to `.bak`, renames `.part` into place, and `exec`s the original argv.
8. If `exec` fails before handoff, the old process restores `.bak`.
9. If the new process starts successfully, it clears pending updater state after Discord `ready`.

## Operator Notes

- First bootstrap to a releasable binary is manual.
- Post-`exec` crash rollback is manual: copy `.bak` back over the executable.
- The executable directory must be writable for `.part`, `.bak`, `.update.lock`, and `.update_state.json`.
- No runtime GitHub token is used.
- Real publishing now comes from `.github/workflows/release.yml` via `workflow_dispatch` on `main`, not from tag pushes.

## Direct Tests in This Task

- env parsing: unset, valid booleans, invalid values, non-Unicode
- download predicates: exact release host/path, exact redirect/final hosts
- checksum/body limits: wrong filename, over-1-KiB checksum, over-limit streamed binary cleanup
- filesystem safety: O_NOFOLLOW state read, atomic state write, reconcile restore
- contention: process-local mutex and cross-process flock
- replacement: rollback when the second rename fails
- self-check: timeout kills and reaps the child

## External Prerequisites Before First Release

- protect `main`
- protect `v*`
- configure the GitHub Actions `release` environment

Those controls live in GitHub settings, not in this repository.
