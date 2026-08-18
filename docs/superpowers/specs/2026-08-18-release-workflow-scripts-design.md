# Release Workflow Script Extraction Design

## Goal

Make `.github/workflows/release.yml` easier to read and maintain by moving its substantive shell and PowerShell logic into focused, version-controlled scripts without changing release behavior.

## Approach

Create focused entrypoints under `scripts/ci/release/` for the workflow's meaningful script blocks:

- version/tag consistency checking
- draft release selection or creation
- Linux package validation
- macOS package validation and startup smoke test
- Windows package validation
- Flatpak build invocation
- verified release asset reconciliation and upload
- draft release validation and publication

The workflow remains responsible for job dependencies, matrices, permissions, action steps, and setup commands. Each extracted script receives workflow-specific values through environment variables so GitHub expression syntax remains in the YAML only where orchestration requires it. The existing `scripts/release-flatpak.sh` remains the implementation of the Flatpak build; its workflow wrapper is extracted for consistency with the other blocks.

## Interfaces and behavior

The scripts run from the repository root and use `set -euo pipefail` for Bash scripts. Matrix-dependent scripts consume explicit environment variables such as `RELEASE_TARGET`, `DEB_ARCH`, and `FLATPAK_ARCH`; release-management scripts consume the existing GitHub-provided variables and `RELEASE_ID`/`GH_TOKEN` values.

The extracted scripts preserve the current package glob patterns, executable checks, architecture checks, startup timeout, GitHub API calls, asset naming rules, stale-asset deletion, duplicate detection, and draft-release safeguards. The YAML calls each script directly and does not duplicate the moved logic.

## Validation and maintenance

Update `scripts/ci/test-pipeline.sh` so its release contracts validate the new script entrypoints and their required behavior rather than requiring implementation details to remain inline in YAML. Add the new release-script paths to the release workflow pull-request path filters. Make Bash entrypoints executable and use PowerShell for the Windows validator.

Verification will include shell syntax checks, PowerShell parse validation where available, the pipeline contract checker, and focused static checks confirming every workflow step points to the intended script and every required script is executable.

## Scope

This is a structural refactor only. It does not alter release triggers, job dependencies, package contents, GitHub permissions, tool versions, artifact names, or publication policy.
