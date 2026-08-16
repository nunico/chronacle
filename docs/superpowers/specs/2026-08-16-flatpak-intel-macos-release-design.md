# Flatpak and Intel macOS Release Design

**Date:** 2026-08-16  
**Status:** Approved

## Goal

Extend Chronacle's existing GitHub release with:

- native Linux aarch64 packages alongside the existing x86_64 packages;
- installable x86_64 and aarch64 Flatpaks, each built from the matching Debian artifact produced
  by the release;
- native macOS x86_64 DMG and app archive assets; and
- fail-closed release publication so a release is not made public before every intended package
  has built and passed its smoke checks.

This is a packaging-only tranche. It does not add Flathub publication, application signing,
notarization, updater support, CSP/devtools hardening, or the broader release-gate restructuring.

## Current state

The tag-triggered release workflow runs a pre-check and then uses `tauri-apps/tauri-action` on
Linux, macOS, and Windows. The current `macos-latest` runner is Apple Silicon, so the workflow
publishes only an arm64 macOS build. Linux publishes AppImage, Debian, and RPM packages. Tauri
does not produce a Flatpak as one of its native bundle targets.

The Debian payload already contains the complete filesystem content the Flatpak needs:

```text
/usr/bin/chronacle
/usr/lib/Chronacle/resources/pdfium/libpdfium.so
/usr/lib/Chronacle/resources/onnxruntime/libonnxruntime.so
/usr/share/applications/Chronacle.desktop
/usr/share/icons/hicolor/*/apps/chronacle.png
```

Repackaging this payload avoids a second Rust/frontend build and guarantees the Flatpak uses the
same application binary and native runtime resources as the Debian release.

## Release architecture

### Native package matrix

Replace the implicit three-runner matrix with explicit entries:

| Platform       | Runner             | Rust target            | Expected packages     |
| -------------- | ------------------ | ---------------------- | --------------------- |
| Linux x86_64   | `ubuntu-24.04`     | native x86_64          | AppImage, Debian, RPM |
| Linux aarch64  | `ubuntu-24.04-arm` | native aarch64         | AppImage, Debian, RPM |
| macOS arm64    | `macos-26`         | `aarch64-apple-darwin` | DMG, app archive      |
| macOS x86_64   | `macos-15-intel`   | `x86_64-apple-darwin`  | DMG, app archive      |
| Windows x86_64 | `windows-2025`     | native x86_64          | MSI, NSIS installer   |

The two macOS jobs pass their target explicitly to Tauri. This prevents a runner-label change from
silently changing an artifact's architecture. The Linux aarch64 job is native rather than
cross-compiled, so its native dependencies and packaged application are exercised on the target
architecture. `macos-15-intel` is scheduled to remain available through August 2027; replacing
that runner is a documented maintenance item rather than an unbounded promise of CI-hosted Intel
builds. `ubuntu-24.04-arm` is a GitHub-hosted public-preview runner, so its availability and image
changes are also an explicit release-maintenance dependency.

Linux aarch64 is included because it provides compatibility for users who cannot rely on a
system-provided x86_64 emulation layer. Native Windows ARM64 remains deferred: Windows 11 on Arm
can run the existing x86_64 package under emulation, while a native package would add another
release target and smoke-test obligation without unlocking the same degree of access.

### Artifact flow

```text
version/tag pre-check
        |
        v
native package matrix (draft GitHub release)
        |
        +-- each Linux job uploads its .deb as an architecture-labelled workflow artifact
        |
        v
Flatpak matrix: matching Debian artifact + validation + smoke test
        |
        v
publish-release job changes the draft to a public release
```

Every native matrix entry must succeed before Flatpak packaging begins. Each Flatpak matrix entry
consumes the matching-architecture Debian workflow artifact, not a public release URL. The final
job publishes the existing draft only when both the native matrix and every Flatpak matrix entry
succeeded. A failed or cancelled dependency leaves the release as a draft.

The release remains non-prerelease unless the tag workflow is changed explicitly in a later design.

## Flatpak package

### Identity and runtime

- Application ID: `dev.tea-driven.chronacle.desktop`, matching `tauri.conf.json`.
- Command: `chronacle`.
- Runtime: `org.gnome.Platform`, version `50`.
- SDK: `org.gnome.Sdk`, version `50`.
- Architectures in this tranche: x86_64 and aarch64.
- Outputs: `Chronacle_<version>_x86_64.flatpak` and
  `Chronacle_<version>_aarch64.flatpak`.
- Runtime repository recorded in the bundle:
  `https://flathub.org/repo/flathub.flatpakrepo`.

The GNOME runtime version is literal and must not use `latest`, `stable`, or another floating
selector. Runtime upgrades are reviewed changes.

### Repository files

Create a focused `packaging/flatpak/` directory:

```text
packaging/flatpak/
├── dev.tea-driven.chronacle.desktop.yml
└── dev.tea-driven.chronacle.desktop.metainfo.xml
```

The YAML manifest accepts a local file named `Chronacle.deb`. Each Flatpak matrix entry copies its
single matching-architecture Debian artifact to that build-input name in a temporary directory
alongside the manifest and metadata. Generated packages, Flatpak repositories, and extracted
Debian contents are never written into the tracked source directory.

The AppStream component uses:

- `desktop-application` component type;
- the same Flatpak ID and desktop launchable ID;
- `AGPL-3.0-only` as the project code license;
- `CC0-1.0` as the metadata license;
- the official Chronacle name, description, repository URL, and branding colors; and
- declared keyboard and pointing-device support.

No screenshots are added in this tranche because the repository has no approved, stable public
screenshot URLs. This does not block a GitHub-hosted Flatpak and avoids inventing metadata.

### Debian extraction

The manifest extracts `data.tar.gz` with `ar` and `tar`, then installs:

- `usr/bin/chronacle` to `/app/bin/chronacle`;
- `usr/lib/Chronacle` to `/app/lib/Chronacle`;
- the generated desktop file to
  `/app/share/applications/dev.tea-driven.chronacle.desktop.desktop` after rewriting its icon to the
  Flatpak ID;
- each available PNG icon to the corresponding hicolor directory under the Flatpak ID; and
- the tracked AppStream file to
  `/app/share/metainfo/dev.tea-driven.chronacle.desktop.metainfo.xml`.

Build commands assert that the binary, PDFium library, ONNX Runtime library, desktop file, and
expected icons exist before installation. A changed Debian layout therefore fails packaging rather
than silently producing an incomplete Flatpak.

## Flatpak sandbox contract

The Flatpak grants only:

```yaml
finish-args:
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri
  - --share=network
```

There is no blanket `--filesystem=home` or `--filesystem=host` permission.

- PDF imports and vault-directory selection continue through Tauri's dialog plugin and the XDG
  file-chooser/document portals. Portal grants remain available across application sessions.
- The selected vault directory remains watchable through its document-portal path.
- Network sharing is required for cloud LLM/embedding providers, local Ollama-compatible endpoints,
  and embedding-model downloads.
- RocksDB data, encrypted settings, models, and caches use Flatpak's normal per-application XDG
  directories under `~/.var/app/dev.tea-driven.chronacle.desktop/`.
- DRI, Wayland, fallback X11, and shared IPC support WebKit rendering across common Linux desktops.

If real-package testing proves that Tauri's directory chooser does not yield a persistent writable
document-portal grant on a supported desktop, the implementation stops for a focused design
revision. It must not respond by silently widening filesystem access.

## Intel macOS behavior

Microsoft does not publish an ONNX Runtime 1.24.2 binary for macOS x86_64. The Intel package
therefore has this explicit support contract:

- PDF ingestion works using the bundled x86_64 PDFium library.
- Cloud embeddings work immediately.
- Local embedding modes are offered only when Chronacle detects a compatible system ONNX Runtime.
- The documented enablement path is `brew install onnxruntime`.
- The package does not copy an unpinned Homebrew library into the app bundle.
- The release workflow does not install ONNX Runtime before its Intel smoke test, ensuring the
  tested default matches a clean user machine.

This preserves the pinned bundled runtime on supported architectures while making the Intel
limitation visible instead of shipping an unofficial or unpinned embedded binary.

## Packaging script and validation

Create `scripts/release-flatpak.sh` as the single local/CI entry point. It takes three positional
arguments:

```text
scripts/release-flatpak.sh <deb-path> <version> <output-dir>
```

The script:

1. validates that the Debian path is one regular file, the version is strict `X.Y.Z` semver, and
   the Debian architecture is either `amd64` or `arm64`;
2. requires `flatpak`, `flatpak-builder`, and `appstreamcli`;
3. prepares a temporary build context and copies the Debian input as `Chronacle.deb`;
4. validates AppStream metadata without network access;
5. validates and builds the manifest against GNOME 50;
6. exports a temporary Flatpak repository;
7. maps Debian `amd64` to Flatpak `x86_64` and Debian `arm64` to Flatpak `aarch64`, then creates the
   correctly named single-file bundle with the Flathub runtime-repository hint;
8. installs the bundle into an isolated per-user test installation;
9. verifies the binary, PDFium, ONNX Runtime, desktop entry, icons, and metadata inside the sandbox;
10. launches Chronacle under a CI display/session bus, confirms the process remains alive through
    the startup window, and then terminates that test process; and
11. writes the completed bundle to the requested output directory.

Temporary paths are created with `mktemp -d` and removed by a scoped exit trap. The script never
deletes a caller-provided directory. It must not print environment variables or settings.

## Workflow validation

Extend `scripts/ci/test-pipeline.sh` before changing the workflow. Tests assert that:

- the release matrix contains both explicit macOS architectures;
- the release matrix contains explicit x86_64 and aarch64 Linux entries;
- the aarch64 Linux entry uses `ubuntu-24.04-arm` and does not cross-compile;
- the Intel entry uses `macos-15-intel` and `x86_64-apple-darwin`;
- the macOS arm entry uses an explicit arm64 target;
- each Linux entry uploads exactly one architecture-labelled Debian workflow artifact;
- Flatpak packaging depends on the complete native matrix;
- the Flatpak matrix matches each Linux architecture to its Debian artifact;
- each Flatpak entry invokes `scripts/release-flatpak.sh` and uploads one architecture-labelled
  `.flatpak` release asset;
- native jobs keep the release in draft state;
- publication depends on both native and Flatpak jobs; and
- only the final publication job changes the release to non-draft.

Manifest-specific validation runs in the Flatpak job because it requires Flatpak tooling and the
real Debian payload. Existing backend, frontend, acceptance, coverage, release-build, and UI E2E
workflows remain otherwise unchanged.

The feature does not add a Gherkin acceptance scenario: it changes distribution infrastructure,
not user-visible application behavior that the backend Playwright harness can exercise. Its
acceptance boundary is the real packaged artifact and release-pipeline test.

## Documentation

Update:

- `README.md` with the supported release matrix, GitHub-release installation instructions for
  Flatpak, and the distinction between Flatpak application storage and portal-selected content;
- `docs/user-guide.md` with the Intel macOS local-embedding limitation and Homebrew enablement; and
- `docs/architecture.md` so its supported-target and release-pipeline descriptions match the five
  native jobs plus the two-entry Flatpak matrix.

The Flatpak instructions use:

```bash
flatpak install --user ./Chronacle_<version>_<architecture>.flatpak
flatpak run dev.tea-driven.chronacle.desktop
```

The documentation gives the concrete architecture values `x86_64` and `aarch64` and tells users
to select the bundle matching their system.

Documentation states that GitHub-hosted single-file bundles do not provide repository-backed
automatic updates. Flathub submission and update delivery remain future work.

## Acceptance criteria

The tranche is complete when:

1. the pipeline-structure test passes;
2. the existing authoritative PR gate passes;
3. release-mode RocksDB tests and the production build pass;
4. the Linux aarch64 job produces aarch64 AppImage, Debian, and RPM artifacts whose binaries report
   the expected architecture;
5. real x86_64 and arm64 Debian artifacts produce valid, installable matching-architecture
   Flatpaks;
6. each installed Flatpak contains and starts with its architecture-correct PDFium and ONNX
   Runtime resources;
7. the Intel macOS job produces x86_64 DMG/app artifacts and passes architecture/startup smoke
   checks without a bundled ONNX Runtime;
8. the arm64 macOS job remains green and continues to bundle ONNX Runtime;
9. a failed Flatpak matrix entry or Intel job demonstrably prevents publication of the draft
   release; and
10. installation and platform limitations are documented without claiming Flathub availability.

Before creating the Chronacle pull request, `scripts/ci/local-pr.sh` must pass. The packaging jobs
that cannot run in the local Docker PR gate must also pass in GitHub Actions before merge.

## Deferred work

- Flathub submission and maintenance automation.
- Native Windows ARM64 packages; the existing x86_64 package remains supported through Windows 11
  on Arm emulation until this is revisited.
- macOS and Windows code signing/notarization.
- Automatic application updates.
- CSP and production-devtools hardening.
- Dependabot/code-scanning enablement.
- Broader release-precheck consolidation with every repository quality gate.
- A replacement for GitHub-hosted Intel macOS builds after August 2027.

## References

- [Tauri Flatpak distribution guide](https://v2.tauri.app/distribute/flatpak/)
- [Flatpak single-file bundle documentation](https://docs.flatpak.org/en/latest/single-file-bundles.html)
- [XDG File Chooser portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html)
- [GitHub-hosted ARM64 runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Windows on Arm FAQ](https://learn.microsoft.com/en-us/windows/arm/faq)
- [GitHub-hosted Intel macOS runner support window](https://github.com/actions/runner-images/issues/13045)
