# Release Workflow Script Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the substantive scripts from the GitHub Actions release workflow into focused repository entrypoints while preserving release behavior and pipeline contracts.

**Architecture:** Add Bash entrypoints under `scripts/ci/release/` and one PowerShell validator for platform-specific checks. The workflow will pass matrix values through step environment variables and invoke these entrypoints; `scripts/ci/test-pipeline.sh` will inspect the extracted source files for canonical release safeguards and inspect YAML only for orchestration and wiring.

**Tech Stack:** Bash, PowerShell, GitHub Actions YAML, Node.js static contract checks, `gh`, `jq`, package inspection tools.

---

### Task 1: Add focused release entrypoints

**Files:**
- Create: `scripts/ci/release/check-version-consistency.sh`
- Create: `scripts/ci/release/create-release.sh`
- Create: `scripts/ci/release/validate-linux-packages.sh`
- Create: `scripts/ci/release/validate-macos-packages.sh`
- Create: `scripts/ci/release/validate-windows-packages.ps1`
- Create: `scripts/ci/release/build-flatpak.sh`
- Create: `scripts/ci/release/upload-release-assets.sh`
- Create: `scripts/ci/release/publish-release.sh`

- [ ] **Step 1: Extract the version and release-management blocks verbatim with explicit interfaces.**

  `check-version-consistency.sh` must read `GITHUB_REF_NAME`, use Node to read `apps/desktop/src-tauri/tauri.conf.json`, and fail unless the tag equals `v${CONF_VERSION}`. `create-release.sh` must retain the existing `gh api --paginate --slurp` lookup, draft-only reuse rule, single-create branch, numeric ID assertion, and `GITHUB_OUTPUT` assignment. `upload-release-assets.sh` and `publish-release.sh` must retain their existing `GH_TOKEN`, `RELEASE_ID`, API endpoints, validation guards, and asset reconciliation order.

- [ ] **Step 2: Extract native and Flatpak validation blocks with environment-based matrix inputs.**

  `validate-linux-packages.sh` must require `RELEASE_TARGET`, `DEB_ARCH`, and `FLATPAK_ARCH`; `validate-macos-packages.sh` must require `RELEASE_TARGET`; and `validate-windows-packages.ps1` must require/read `RELEASE_TARGET` only as needed for the bundle root. Preserve the current globs, count checks, executable checks, architecture checks, resource checks, and ten-second startup smoke behavior. `build-flatpak.sh` must read the version from `apps/desktop/src-tauri/tauri.conf.json` and invoke `scripts/release-flatpak.sh artifacts/*.deb "$VERSION" flatpak-out`.

- [ ] **Step 3: Set executable permissions and validate script syntax.**

  Run:

  ```bash
  chmod +x scripts/ci/release/*.sh
  bash -n scripts/ci/release/*.sh
  ```

  Expected: both commands exit successfully; the PowerShell file remains non-executable and is checked separately when `pwsh` is available.

### Task 2: Replace inline workflow bodies with script calls

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Replace each extracted `run: |` body with a direct script invocation.**

  Keep setup/install commands inline. Use these interfaces:

  ```yaml
  run: scripts/ci/release/check-version-consistency.sh
  run: scripts/ci/release/create-release.sh
  run: scripts/ci/release/validate-linux-packages.sh
  run: scripts/ci/release/validate-macos-packages.sh
  shell: pwsh
  run: ./scripts/ci/release/validate-windows-packages.ps1
  run: scripts/ci/release/build-flatpak.sh
  run: scripts/ci/release/upload-release-assets.sh
  run: scripts/ci/release/publish-release.sh
  ```

  Pass `RELEASE_TARGET`, `DEB_ARCH`, and `FLATPAK_ARCH` through each relevant step's `env` mapping using the existing matrix expressions. Preserve step names, IDs, conditions, job dependencies, permissions, artifact paths, and release outputs.

- [ ] **Step 2: Add all new release-script paths to the pull-request path filter.**

  Include `scripts/ci/release/**` alongside the existing release workflow and release-script paths. Do not change unrelated trigger paths.

- [ ] **Step 3: Inspect the resulting YAML for accidental inline release logic.**

  Run:

  ```bash
  rg -n 'run: \||gh api|jq|dpkg-deb|rpm2cpio|flatpak-builder|Chronacle_\*' .github/workflows/release.yml
  ```

  Expected: only setup commands and the direct script calls remain; release API, package validation, and Flatpak build logic are absent from the YAML.

### Task 3: Move pipeline contracts to the extracted sources

**Files:**
- Modify: `scripts/ci/test-pipeline.sh`

- [ ] **Step 1: Add source-file loading and release-script path constants.**

  Read the eight extracted script files with `fs.readFileSync`, fail with a clear contract message if any is missing, and define a helper that checks each Bash entrypoint is executable. Keep existing fixture tests for `canonicalCreateFlow`, `canonicalAssetUpload`, and `canonicalPublish` so the parser algorithms continue to reject unsafe variants.

- [ ] **Step 2: Apply canonical checks to the extracted implementations.**

  Replace checks that pass inline workflow bodies to `canonicalCreateFlow`, `canonicalAssetUpload`, `canonicalPublish`, and `canonicalLinuxValidation` with checks against the corresponding script source. Add checks for the version script, macOS validator, Windows validator, Flatpak wrapper, and required environment variables. The YAML checks must instead assert the expected direct invocation, shell, environment wiring, and preserved job/step guards.

- [ ] **Step 3: Update release workflow path coverage and run the contract checker.**

  Add a contract requiring `.github/workflows/release.yml` to include `scripts/ci/release/**` in its pull-request paths, then run:

  ```bash
  scripts/ci/test-pipeline.sh
  ```

  Expected: `pipeline contract passed` (or the repository's existing successful output) with exit code 0.

### Task 4: Verify the refactor

**Files:**
- Verify: `.github/workflows/release.yml`
- Verify: `scripts/ci/release/*`
- Verify: `scripts/ci/test-pipeline.sh`

- [ ] **Step 1: Run static and syntax checks.**

  ```bash
  git diff --check
  bash -n scripts/ci/release/*.sh
  if command -v pwsh >/dev/null 2>&1; then
    pwsh -NoProfile -Command "\$null = [System.Management.Automation.Language.Parser]::ParseFile('scripts/ci/release/validate-windows-packages.ps1', [ref]\$null, [ref]\$null)"
  fi
  scripts/ci/test-pipeline.sh
  ```

  Expected: all commands exit 0.

- [ ] **Step 2: Review the diff against the approved scope.**

  ```bash
  git diff --stat
  git diff -- .github/workflows/release.yml scripts/ci/release scripts/ci/test-pipeline.sh
  git status --short
  ```

  Confirm that only the workflow, release entrypoints, and pipeline contract are changed after the already-committed design and that no lockfiles, generated files, or credentials are touched.

- [ ] **Step 3: Commit the implementation.**

  ```bash
  git add .github/workflows/release.yml scripts/ci/release scripts/ci/test-pipeline.sh
  git commit -m "refactor: extract release workflow scripts"
  ```
