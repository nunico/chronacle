# Release Pipeline Design

**Date:** 2026-06-08
**Status:** Approved

## Goal

Automate building and publishing installable packages for macOS, Windows, and Linux whenever a versioned git tag is pushed. Artifacts are published as assets on a GitHub Release.

## Trigger

```yaml
on:
  push:
    tags:
      - 'v[0-9]+.[0-9]+.[0-9]+'
```

Semantic-version tags only (e.g. `v0.2.0`). Loose or non-semver tags do not trigger the pipeline.

## Workflow File

New file: `.github/workflows/release.yml` — independent of the existing `ci.yml`.

## Job Structure

### Job 1: `pre-check` (ubuntu-latest)

Runs before any build starts. Fails fast to prevent shipping a broken build.

Steps:
1. Checkout
2. Install Rust toolchain (stable, with `rustfmt` + `clippy` components)
3. Install system deps (WebKit2GTK, GTK3, etc.)
4. Install Node + pnpm
5. `pnpm install`
6. **Version consistency check** — extract `version` from `src-tauri/tauri.conf.json`, compare to `${{ github.ref_name }}` (e.g. `v0.2.0`). Fail if they diverge (tag must equal `v<tauri.conf version>`).
7. `cargo fmt --check`
8. `cargo clippy --all-targets --all-features -- -D warnings`
9. `cargo test`
10. `pnpm typecheck`
11. `pnpm lint`
12. `pnpm test --run`

### Job 2: `build` (matrix)

`needs: pre-check`. Runs in parallel across three runners:

| Runner | Bundles produced |
|--------|-----------------|
| `ubuntu-latest` | `.deb`, `.AppImage`, `.rpm` |
| `macos-latest` | `.dmg`, `.app.tar.gz` |
| `windows-latest` | `.msi`, `.exe` (NSIS) |

Each job uses `tauri-apps/tauri-action`. The ubuntu job creates the GitHub Release; mac and windows jobs upload to it. All use the same `tagName: ${{ github.ref_name }}`.

No code signing. `GITHUB_TOKEN` (automatically available) is the only secret required.

## Bundle Configuration

`src-tauri/tauri.conf.json` already has `"targets": "all"` — no changes needed.

## Versioning Convention

The version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` must be bumped and committed before tagging. The workflow enforces this with the version consistency check in `pre-check`.

**Release workflow for maintainers:**
1. Bump `version` in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`
2. Commit: `git commit -m "chore: bump version to X.Y.Z"`
3. Tag: `git tag vX.Y.Z && git push --tags`

The GitHub Release is created automatically. Edit the release notes on GitHub after the workflow completes.

## What Is Not In Scope

- Code signing (deferred; will require Apple Developer and/or Windows cert secrets when added)
- Auto-generating changelogs or release notes
- Publishing to package registries (Homebrew, winget, AUR)
- Auto-bumping the version (no release-please or semantic-release)
