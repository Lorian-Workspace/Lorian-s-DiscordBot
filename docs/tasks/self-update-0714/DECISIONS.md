# Self-Update & Fixed Owner — Decisions

## Scope

- Fixed owner authority: one hardcoded `OWNER_ID` in `src/config.rs`
- Linux x86_64 self-update from this repository's GitHub releases
- Owner-only `/update` command plus release-build background checks
- GitHub Actions release publishing from a trusted `main` workflow run

## Trust Model

- Runtime trust root: repository `Lorian-Workspace/Lorian-s-DiscordBot`, GitHub TLS, the default-branch release workflow, and SHA256 verification
- Non-goal: publisher signatures; if the GitHub repo or release workflow is compromised, the checksum can be compromised too
- No runtime GitHub token, no user-supplied URL, no repo override, no TOML authority override

## Owner Authority

- `src/config.rs::OWNER_ID` is the only authorization source for `/update` and other privileged checks
- `data/owner_info.toml` is display/prompt metadata only; its `discord_id` is not trusted for authorization
- Unauthorized `/update` requests are denied before deferral or updater side effects

## Update Flow

- Auto-update runs only in release builds
- `AUTO_UPDATE_ENABLED` semantics:
  - unset => enabled
  - `true`/`yes`/`1` => enabled
  - `false`/`no`/`0` => disabled
  - invalid or non-Unicode => log `ERROR`, disable
- Update downloads are limited to the configured release asset path on `github.com`
- Redirects and final URLs are accepted only for exact allowlisted GitHub hosts over HTTPS
- The updater verifies the SHA256 file, including the exact asset filename, before replace
- The staged binary is executed with `--version`; timeout kills and waits for the child before failing
- Replace uses `.part` + `.bak` in the executable directory and requires that directory to be writable
- If `exec` fails before the new process takes over, the old process restores `.bak`
- If the new process starts and later crashes, rollback is manual; `.bak` is kept for recovery

## Release Workflow

- Publishing is `workflow_dispatch` only, from the repository default branch
- The workflow rejects non-`main` dispatches, non-stable SemVer input, non-HEAD runs, Cargo-version mismatches, and pre-existing tags
- Build/test runs with read-only `contents: read`
- Publish runs separately with `contents: write` and the protected `release` environment

## Required External Controls

- Protect `main`
- Protect `v*` tags with a ruleset
- Configure and protect the GitHub Actions `release` environment

These are blocking operational prerequisites for the first real release. They are not enforced from inside this repository.

## Direct Test Coverage Added Here

- auto-update env parsing, including invalid/non-Unicode disable behavior
- exact-host release URL and redirect/final URL validation
- checksum filename mismatch and checksum size-cap enforcement
- streamed binary size-cap cleanup
- O_NOFOLLOW symlink rejection for persisted updater state
- atomic state write and startup reconcile restore
- process-local and cross-process lock contention
- replace failure rollback to `.bak`
- self-check timeout child kill/wait cleanup
