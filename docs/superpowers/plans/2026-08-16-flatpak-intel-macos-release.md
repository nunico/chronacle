# Flatpak, Linux ARM64, and Intel macOS Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish tested x86_64 and aarch64 Linux packages and Flatpaks, native Apple Silicon and
Intel macOS packages, and Windows x86_64 packages through a fail-closed draft-release workflow.

**Architecture:** Replace the implicit release runner matrix with explicit target records. Linux
jobs pass their architecture-labelled Debian packages to a matching Flatpak matrix, which
repackages the exact Debian payload without rebuilding Chronacle; a final job publishes the draft
only after all native and Flatpak checks succeed. Existing runtime selection remains unchanged:
Linux bundles architecture-matched PDFium and ONNX Runtime, while Intel macOS bundles PDFium and
uses the existing system/Homebrew ONNX Runtime fallback.

**Tech Stack:** GitHub Actions, Tauri 2, POSIX shell, Flatpak Builder, GNOME Platform/SDK 50,
AppStream metadata, Debian package tools, Rust unit tests

**Design:**
`docs/superpowers/specs/2026-08-16-flatpak-intel-macos-release-design.md`

---

## File map

- Modify `apps/desktop/src-tauri/src/runtime_downloads.rs` to expose pure target-to-runtime asset
  selection.
- Modify `apps/desktop/src-tauri/src/runtime_downloads_tests.rs` to lock Linux aarch64 and Intel
  macOS runtime behavior.
- Modify `apps/desktop/src-tauri/build.rs` to consume the tested target-selection helpers.
- Create `packaging/flatpak/dev.tea-driven.chronacle.desktop.yml` as the architecture-neutral
  Flatpak manifest.
- Create `packaging/flatpak/dev.tea-driven.chronacle.desktop.metainfo.xml` as AppStream metadata.
- Create `scripts/release-flatpak.sh` as the only local and CI Flatpak build/smoke-test entry point.
- Create `scripts/ci/test-release-flatpak.sh` for argument, architecture, and output-name contract
  tests with stubbed external tools.
- Modify `scripts/ci/test-pipeline.sh` to enforce the native/Flatpak matrix and fail-closed release
  topology.
- Modify `scripts/ci/backend-quality.sh` so the Flatpak script contract runs in the PR gate.
- Modify `.github/workflows/release.yml` to build five native targets, two Flatpaks, and publish only
  after all checks pass.
- Modify `README.md`, `docs/user-guide.md`, and `docs/architecture.md` to document packages,
  installation, storage, and platform limitations.

### Task 1: Lock the native runtime support matrix

**Files:**

- Modify: `apps/desktop/src-tauri/src/runtime_downloads.rs`
- Modify: `apps/desktop/src-tauri/src/runtime_downloads_tests.rs`
- Modify: `apps/desktop/src-tauri/build.rs`

- [ ] **Step 1: Write failing target-selection tests**

Add tests that express the packaging contract without downloading either runtime:

```rust
#[test]
fn linux_runtime_assets_cover_x86_64_and_aarch64() {
    assert_eq!(
        pdfium_asset("linux", "x86_64"),
        Some(("pdfium-linux-x64.tgz", "libpdfium.so"))
    );
    assert_eq!(
        pdfium_asset("linux", "aarch64"),
        Some(("pdfium-linux-arm64.tgz", "libpdfium.so"))
    );
    assert_eq!(
        onnxruntime_asset("linux", "x86_64", "1.24.2"),
        Some(("onnxruntime-linux-x64-1.24.2.tgz".into(), ArchiveKind::Tar,
              "libonnxruntime.so"))
    );
    assert_eq!(
        onnxruntime_asset("linux", "aarch64", "1.24.2"),
        Some(("onnxruntime-linux-aarch64-1.24.2.tgz".into(), ArchiveKind::Tar,
              "libonnxruntime.so"))
    );
}

#[test]
fn intel_macos_bundles_pdfium_but_not_onnxruntime() {
    assert_eq!(
        pdfium_asset("macos", "x86_64"),
        Some(("pdfium-mac-x64.tgz", "libpdfium.dylib"))
    );
    assert_eq!(onnxruntime_asset("macos", "x86_64", "1.24.2"), None);
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p Chronacle runtime_downloads -- --nocapture
```

Expected: compilation fails because `pdfium_asset`, `onnxruntime_asset`, and `ArchiveKind` do not
exist in `runtime_downloads.rs`.

- [ ] **Step 3: Add pure runtime asset selectors**

Add a copyable archive enum and pure functions to `runtime_downloads.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArchiveKind {
    Tar,
    Zip,
}

pub(crate) fn pdfium_asset(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    match (os, arch) {
        ("macos", "aarch64") => Some(("pdfium-mac-arm64.tgz", "libpdfium.dylib")),
        ("macos", "x86_64") => Some(("pdfium-mac-x64.tgz", "libpdfium.dylib")),
        ("linux", "x86_64") => Some(("pdfium-linux-x64.tgz", "libpdfium.so")),
        ("linux", "aarch64") => Some(("pdfium-linux-arm64.tgz", "libpdfium.so")),
        ("windows", "x86_64") => Some(("pdfium-win-x64.tgz", "pdfium.dll")),
        _ => None,
    }
}

pub(crate) fn onnxruntime_asset(
    os: &str,
    arch: &str,
    version: &str,
) -> Option<(String, ArchiveKind, &'static str)> {
    match (os, arch) {
        ("macos", "aarch64") => Some((
            format!("onnxruntime-osx-arm64-{version}.tgz"),
            ArchiveKind::Tar,
            "libonnxruntime.dylib",
        )),
        ("linux", "x86_64") => Some((
            format!("onnxruntime-linux-x64-{version}.tgz"),
            ArchiveKind::Tar,
            "libonnxruntime.so",
        )),
        ("linux", "aarch64") => Some((
            format!("onnxruntime-linux-aarch64-{version}.tgz"),
            ArchiveKind::Tar,
            "libonnxruntime.so",
        )),
        ("windows", "x86_64") => Some((
            format!("onnxruntime-win-x64-{version}.zip"),
            ArchiveKind::Zip,
            "onnxruntime.dll",
        )),
        ("windows", "aarch64") => Some((
            format!("onnxruntime-win-arm64-{version}.zip"),
            ArchiveKind::Zip,
            "onnxruntime.dll",
        )),
        _ => None,
    }
}
```

Replace the duplicated matches and private `Archive` enum in `build.rs` with these helpers. Keep
the current unsupported-target warnings and map `ArchiveKind::Tar`/`Zip` to the existing extraction
functions.

- [ ] **Step 4: Run formatting, lint, and focused tests**

Run:

```bash
cargo fmt --all
cargo test -p Chronacle runtime_downloads -- --nocapture
cargo clippy -p Chronacle --all-targets --features rocksdb -- -D warnings
```

Expected: all runtime download tests pass and Clippy exits zero.

- [ ] **Step 5: Commit the runtime contract**

```bash
git add apps/desktop/src-tauri/build.rs \
  apps/desktop/src-tauri/src/runtime_downloads.rs \
  apps/desktop/src-tauri/src/runtime_downloads_tests.rs
git commit -m "test: lock release runtime architecture support"
```

### Task 2: Add Flatpak metadata and manifest contracts

**Files:**

- Create: `packaging/flatpak/dev.tea-driven.chronacle.desktop.yml`
- Create: `packaging/flatpak/dev.tea-driven.chronacle.desktop.metainfo.xml`
- Create: `scripts/ci/test-release-flatpak.sh`
- Modify: `scripts/ci/backend-quality.sh`

- [ ] **Step 1: Write a failing Flatpak contract test**

Create an executable POSIX shell test that checks the required files, exact runtime, app ID,
command, permissions, Debian source name, metadata IDs, and absence of broad filesystem access:

```sh
#!/bin/sh
set -eu

manifest=packaging/flatpak/dev.tea-driven.chronacle.desktop.yml
metadata=packaging/flatpak/dev.tea-driven.chronacle.desktop.metainfo.xml

test -f "$manifest"
test -f "$metadata"
rg -q '^app-id: dev\.tea-driven\.chronacle\.desktop$' "$manifest"
rg -q '^runtime: org\.gnome\.Platform$' "$manifest"
rg -q '^runtime-version: "50"$' "$manifest"
rg -q '^sdk: org\.gnome\.Sdk$' "$manifest"
rg -q '^command: chronacle$' "$manifest"
rg -q 'path: Chronacle\.deb$' "$manifest"
rg -q -- '--socket=wayland' "$manifest"
rg -q -- '--socket=fallback-x11' "$manifest"
rg -q -- '--share=ipc' "$manifest"
rg -q -- '--device=dri' "$manifest"
rg -q -- '--share=network' "$manifest"
! rg -q -- '--filesystem=(home|host)' "$manifest"
rg -q '<id>dev\.tea-driven\.chronacle\.desktop</id>' "$metadata"
rg -q '<launchable type="desktop-id">dev\.tea-driven\.chronacle\.desktop\.desktop</launchable>' \
  "$metadata"
```

- [ ] **Step 2: Run it and verify the missing files fail the test**

Run: `scripts/ci/test-release-flatpak.sh`

Expected: FAIL at the first `test -f` because the Flatpak files do not exist.

- [ ] **Step 3: Create the architecture-neutral manifest**

Use this manifest structure, with assertions before installation:

```yaml
app-id: dev.tea-driven.chronacle.desktop
runtime: org.gnome.Platform
runtime-version: "50"
sdk: org.gnome.Sdk
command: chronacle
finish-args:
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri
  - --share=network
modules:
  - name: chronacle
    buildsystem: simple
    build-commands:
      - ar x Chronacle.deb
      - mkdir deb-root
      - tar -xzf data.tar.gz -C deb-root
      - test -x deb-root/usr/bin/chronacle
      - test -f deb-root/usr/lib/Chronacle/resources/pdfium/libpdfium.so
      - test -f deb-root/usr/lib/Chronacle/resources/onnxruntime/libonnxruntime.so
      - install -Dm755 deb-root/usr/bin/chronacle /app/bin/chronacle
      - cp -a deb-root/usr/lib/Chronacle /app/lib/Chronacle
      - install -Dm644 dev.tea-driven.chronacle.desktop.metainfo.xml /app/share/metainfo/dev.tea-driven.chronacle.desktop.metainfo.xml
      - install -Dm644 deb-root/usr/share/applications/Chronacle.desktop /app/share/applications/dev.tea-driven.chronacle.desktop.desktop
      - sed -i 's/^Icon=.*/Icon=dev.tea-driven.chronacle.desktop/' /app/share/applications/dev.tea-driven.chronacle.desktop.desktop
      - for icon in deb-root/usr/share/icons/hicolor/*/apps/chronacle.png; do size=$(basename "$(dirname "$(dirname "$icon")")"); install -Dm644 "$icon" "/app/share/icons/hicolor/$size/apps/dev.tea-driven.chronacle.desktop.png"; done
    sources:
      - type: file
        path: Chronacle.deb
      - type: file
        path: dev.tea-driven.chronacle.desktop.metainfo.xml
```

- [ ] **Step 4: Create complete AppStream metadata**

Create this `desktop-application` component; do not add screenshot URLs:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>dev.tea-driven.chronacle.desktop</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>AGPL-3.0-only</project_license>
  <name>Chronacle</name>
  <summary>A local-first TTRPG game-master assistant</summary>
  <description>
    <p>Load rulebook PDFs, keep structured campaign notes, and ask questions with source citations.</p>
  </description>
  <launchable type="desktop-id">dev.tea-driven.chronacle.desktop.desktop</launchable>
  <developer id="dev.tea-driven">
    <name>Tea Driven</name>
  </developer>
  <url type="homepage">https://github.com/nunico/chronacle</url>
  <url type="bugtracker">https://github.com/nunico/chronacle/issues</url>
  <branding>
    <color type="primary" scheme_preference="light">#3d5bff</color>
    <color type="primary" scheme_preference="dark">#05060f</color>
  </branding>
  <supports>
    <control>keyboard</control>
    <control>pointing</control>
  </supports>
  <content_rating type="oars-1.1" />
</component>
```

- [ ] **Step 5: Wire the contract test into backend quality and run it**

Add this line immediately after `scripts/ci/test-pipeline.sh` in `scripts/ci/backend-quality.sh`:

```sh
scripts/ci/test-release-flatpak.sh
```

Run:

```bash
chmod +x scripts/ci/test-release-flatpak.sh
scripts/ci/test-release-flatpak.sh
mise exec -- pnpm exec prettier --check packaging/flatpak scripts/ci/test-release-flatpak.sh
```

Expected: the contract test and formatting check pass.

- [ ] **Step 6: Commit the manifest and metadata**

```bash
git add packaging/flatpak scripts/ci/test-release-flatpak.sh scripts/ci/backend-quality.sh
git commit -m "feat: define Flatpak package metadata"
```

### Task 3: Build and smoke-test Flatpaks through one script

**Files:**

- Create: `scripts/release-flatpak.sh`
- Modify: `scripts/ci/test-release-flatpak.sh`

- [ ] **Step 1: Add failing CLI validation tests**

Extend `test-release-flatpak.sh` with a temporary directory and assertions for these invocations:

```sh
if scripts/release-flatpak.sh 2>/dev/null; then
  echo 'release-flatpak must reject missing arguments' >&2
  exit 1
fi

touch "$test_root/not-a-deb.txt"
if scripts/release-flatpak.sh "$test_root/not-a-deb.txt" 1.2.3 "$test_root/out" 2>/dev/null; then
  echo 'release-flatpak must reject a non-Debian input' >&2
  exit 1
fi

if scripts/release-flatpak.sh "$test_root/not-a-deb.txt" v1.2.3 "$test_root/out" 2>/dev/null; then
  echo 'release-flatpak must reject prefixed semver' >&2
  exit 1
fi
```

Add stub executables under `$test_root/bin` for `dpkg-deb`, `flatpak`, `flatpak-builder`, and
`appstreamcli`. The `dpkg-deb` stub returns `amd64` and then `arm64`; the other stubs record their
arguments and create the expected repository/bundle paths. Assert that successful stubbed runs
produce `Chronacle_1.2.3_x86_64.flatpak` and `Chronacle_1.2.3_aarch64.flatpak` and never delete the
caller-owned output directory.

- [ ] **Step 2: Run the test and verify the missing entry point fails**

Run: `scripts/ci/test-release-flatpak.sh`

Expected: FAIL because `scripts/release-flatpak.sh` does not exist.

- [ ] **Step 3: Implement strict input and tool validation**

Create `scripts/release-flatpak.sh` with `#!/bin/sh` and `set -eu`. Require exactly three
arguments, a regular `.deb` file, strict `X.Y.Z` semver, and an output path that is a directory or
can be created as one. Use `dpkg-deb -f "$deb_path" Architecture` and accept only:

```sh
case "$deb_arch" in
  amd64) flatpak_arch=x86_64 ;;
  arm64) flatpak_arch=aarch64 ;;
  *) fail "unsupported Debian architecture: $deb_arch" ;;
esac
```

Resolve repository-relative inputs from `git rev-parse --show-toplevel`, require `flatpak`,
`flatpak-builder`, `appstreamcli`, `dpkg-deb`, `ar`, and `tar`, and create all scratch state with
`mktemp -d`. The exit trap must remove only that resolved temporary directory.

- [ ] **Step 4: Implement build, install, inspection, and startup smoke**

Copy the manifest, metadata, and Debian input into the temporary context. Then execute the
equivalent of:

```sh
appstreamcli validate --no-net "$context/dev.tea-driven.chronacle.desktop.metainfo.xml"
flatpak-builder --force-clean --arch="$flatpak_arch" \
  --repo="$repo" "$build_dir" "$context/dev.tea-driven.chronacle.desktop.yml"
flatpak build-bundle --arch="$flatpak_arch" \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  "$repo" "$bundle" dev.tea-driven.chronacle.desktop
```

Set a temporary `XDG_DATA_HOME` for the test installation, install the bundle with
`flatpak --user --noninteractive install`, and assert all of these inside the sandbox:

```text
/app/bin/chronacle
/app/lib/Chronacle/resources/pdfium/libpdfium.so
/app/lib/Chronacle/resources/onnxruntime/libonnxruntime.so
/app/share/applications/dev.tea-driven.chronacle.desktop.desktop
/app/share/metainfo/dev.tea-driven.chronacle.desktop.metainfo.xml
```

Verify `flatpak info --user --show-arch dev.tea-driven.chronacle.desktop` equals `$flatpak_arch`.
Launch through `dbus-run-session -- xvfb-run -a flatpak run`, poll until the process has remained
alive for ten seconds, terminate it, wait for it, and treat an early exit as failure. Move only the
finished bundle to the caller's output directory.

- [ ] **Step 5: Run the script contract test**

Run:

```bash
chmod +x scripts/release-flatpak.sh
scripts/ci/test-release-flatpak.sh
```

Expected: all validation and architecture/output-name cases pass with stubbed external tools.

- [ ] **Step 6: Commit the packaging entry point**

```bash
git add scripts/release-flatpak.sh scripts/ci/test-release-flatpak.sh
git commit -m "feat: build and smoke-test Flatpak bundles"
```

### Task 4: Specify the release workflow contract

**Files:**

- Modify: `scripts/ci/test-pipeline.sh`

- [ ] **Step 1: Add failing native matrix assertions**

Add a Node heredoc that reads `.github/workflows/release.yml` as text and rejects missing explicit
matrix records. The assertions must look for these exact runner/target pairs:

```text
ubuntu-24.04 / x86_64-unknown-linux-gnu
ubuntu-24.04-arm / aarch64-unknown-linux-gnu
macos-26 / aarch64-apple-darwin
macos-15-intel / x86_64-apple-darwin
windows-2025 / x86_64-pc-windows-msvc
```

Also require `releaseDraft: true`, architecture-labelled Debian artifact uploads for both Linux
entries, inspectable native workflow artifacts, explicit Tauri `--target` arguments for both macOS
entries, and a path-filtered `pull_request` trigger that exercises packaging changes without
publishing a release.

- [ ] **Step 2: Add failing Flatpak and publication assertions**

In the same parser, require:

```text
flatpak.needs = build
flatpak matrix contains x86_64 and aarch64
flatpak invokes scripts/release-flatpak.sh
flatpak uploads *.flatpak to the draft release
publish-release.needs contains build and flatpak
publish-release runs gh release edit ... --draft=false
```

Assert that no build or Flatpak step contains `releaseDraft: false` and that only
`publish-release` can remove draft status. Require release upload and publication steps to be
guarded by `startsWith(github.ref, 'refs/tags/v')`. Parse job boundaries rather than counting an
unscoped string across the file.

- [ ] **Step 3: Run the pipeline contract and verify it fails**

Run: `scripts/ci/test-pipeline.sh`

Expected: FAIL because the current release workflow has an implicit three-runner matrix, publishes
immediately, and has no Flatpak or final publication jobs.

- [ ] **Step 4: Commit the red contract test**

```bash
git add scripts/ci/test-pipeline.sh
git commit -m "test: specify multi-architecture release topology"
```

### Task 5: Implement the fail-closed release workflow

**Files:**

- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Replace the implicit build matrix**

Add a `pull_request` trigger limited to the release workflow, Flatpak packaging files, Flatpak
scripts, runtime-download selection files, and the three release-facing documentation files. The
tag trigger remains unchanged. Guard tag/version consistency, Tauri GitHub-release inputs, Flatpak
release upload, and final publication so pull requests build and smoke-test artifacts without
creating or editing a GitHub release.

Define explicit matrix records with `name`, `os`, `target`, `deb_arch`, and `flatpak_arch` fields.
Use stable names `linux-x86_64`, `linux-aarch64`, `macos-arm64`, `macos-x86_64`, and
`windows-x86_64` for the five rows from Task 4, setting the Debian/Flatpak fields only on Linux
records. Set `runs-on: ${{ matrix.os }}` and pass this Tauri argument for every record:

```yaml
args: --target ${{ matrix.target }} --features rocksdb
```

Keep dependency installation conditional on `runner.os`, keep action versions already used by the
repository, and set `releaseDraft: true` on `tauri-apps/tauri-action@v0`. Set `tagName` and
`releaseName` to empty strings for pull-request runs so the action only builds packages.

- [ ] **Step 2: Add native artifact checks**

After Tauri packaging, use platform-specific steps:

- Linux: locate exactly one `.deb`, `.AppImage`, and `.rpm`; run `file` on their extracted or direct
  executable payloads; fail unless they match the matrix architecture; upload the Debian file as
  `chronacle-deb-${{ matrix.flatpak_arch }}` with `actions/upload-artifact@v4`.
- macOS: locate exactly one `Chronacle.app`, `.app.tar.gz`, and `.dmg`; verify the app executable
  with `file`; assert PDFium is present; assert ONNX Runtime is present for arm64 and absent for
  x86_64; launch the executable for ten seconds; and fail on early exit.
- Windows: retain the existing x86_64 Tauri bundle result and assert exactly one MSI and one NSIS
  installer were produced.

Upload every matrix row's complete validated bundle directory as
`chronacle-native-${{ matrix.name }}` with `actions/upload-artifact@v4`. These workflow artifacts
make pull-request packaging runs inspectable; tag runs additionally upload the same bundles to the
draft GitHub release through `tauri-action`.

- [ ] **Step 3: Add the architecture-matched Flatpak matrix**

Create `flatpak` with `needs: build`, `permissions: contents: write`, `fail-fast: false`, and rows:

```yaml
include:
  - os: ubuntu-24.04
    arch: x86_64
  - os: ubuntu-24.04-arm
    arch: aarch64
```

Install `flatpak`, `flatpak-builder`, `appstream`, `xvfb`, and `dbus-x11`; add Flathub from
the literal repository URL; install `org.gnome.Platform//50` and `org.gnome.Sdk//50` for the matrix
architecture; download only `chronacle-deb-${{ matrix.arch }}`; and run:

```bash
scripts/release-flatpak.sh artifacts/*.deb "${GITHUB_REF_NAME#v}" flatpak-out
gh release upload "$GITHUB_REF_NAME" flatpak-out/*.flatpak --clobber
```

Supply `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` only to the upload step, and run that step only for a
matching release tag. Always upload both Flatpak bundles as workflow artifacts so pull-request runs
retain inspectable results.

- [ ] **Step 4: Add the final publication job**

Create a `publish-release` job with `needs: [build, flatpak]`, `runs-on: ubuntu-24.04`, and
`contents: write`. Guard the entire job with
`startsWith(github.ref, 'refs/tags/v')`. Check out no source code; publish only with:

```bash
gh release edit "$GITHUB_REF_NAME" --draft=false --prerelease=false
```

This job must have no `always()` override, so a failed or cancelled dependency prevents it from
running and leaves the release draft intact.

- [ ] **Step 5: Run the workflow contract and syntax checks**

Run:

```bash
scripts/ci/test-pipeline.sh
mise exec -- pnpm exec prettier --check .github/workflows/release.yml
```

Expected: the pipeline contract passes and Prettier reports the workflow formatted.

- [ ] **Step 6: Commit the workflow**

```bash
git add .github/workflows/release.yml
git commit -m "feat: release Linux ARM64 and Flatpak packages"
```

### Task 6: Validate real Flatpak packages on both architectures

**Files:**

- Modify if validation exposes a defect: `packaging/flatpak/dev.tea-driven.chronacle.desktop.yml`
- Modify if validation exposes a defect: `scripts/release-flatpak.sh`
- Modify if validation exposes a defect: `scripts/ci/test-release-flatpak.sh`

- [ ] **Step 1: Build the x86_64 Debian package**

On x86_64 Linux with GNOME Platform/SDK 50 installed, run:

```bash
mise exec -- pnpm -C apps/desktop exec tauri build --bundles deb --features rocksdb
```

Expected: exactly one Debian package under
`apps/desktop/src-tauri/target/release/bundle/deb/`.

- [ ] **Step 2: Build and inspect the x86_64 Flatpak**

Run:

```bash
scripts/release-flatpak.sh \
  apps/desktop/src-tauri/target/release/bundle/deb/*.deb \
  0.3.0 \
  target/flatpak-smoke
```

Expected: the script completes its install/resource/startup smoke test and emits the x86_64 bundle
using GNOME 50.

- [ ] **Step 3: Exercise both architectures in the pull request**

Push the feature branch and open or update its pull request. The path-filtered release workflow
runs the five native matrix entries and two Flatpak entries without creating a GitHub release.
Verify the `ubuntu-24.04-arm` job produces ARM64 Debian/AppImage/RPM workflow artifacts and that its
Flatpak job completes the same resource and startup checks natively.

Expected: both Linux matrix rows, both Flatpak rows, both macOS rows, and Windows x86_64 pass; the
`publish-release` job is skipped and no release is created.

- [ ] **Step 4: Commit only fixes required by real-package validation**

If files changed, run the script contract again and commit them:

```bash
scripts/ci/test-release-flatpak.sh
git add packaging/flatpak scripts/release-flatpak.sh scripts/ci/test-release-flatpak.sh
git commit -m "fix: harden Flatpak package validation"
```

If validation required no changes, record the successful workflow URLs for the pull-request test
report and make no empty commit.

### Task 7: Document installation and platform behavior

**Files:**

- Modify: `README.md`
- Modify: `docs/user-guide.md`
- Modify: `docs/architecture.md`

- [ ] **Step 1: Add the release package matrix to README**

Document these supported artifacts:

| Platform | Architectures        | Packages                                                  |
| -------- | -------------------- | --------------------------------------------------------- |
| Linux    | x86_64, aarch64      | AppImage, Debian, RPM, Flatpak bundle                     |
| macOS    | Apple Silicon, Intel | DMG, app archive                                          |
| Windows  | x86_64               | MSI, NSIS; usable on Windows 11 Arm through x64 emulation |

Add GitHub-release Flatpak installation commands using concrete filenames for both architectures:

```bash
flatpak install --user ./Chronacle_<version>_x86_64.flatpak
flatpak install --user ./Chronacle_<version>_aarch64.flatpak
flatpak run dev.tea-driven.chronacle.desktop
```

State that these bundles are not a Flathub listing and do not receive repository-backed automatic
updates. Explain that app data lives under Flatpak's per-application storage and selected PDFs or
vault directories are accessed through portal grants.

- [ ] **Step 2: Document Intel macOS embeddings in the user guide**

State plainly that PDF ingestion and cloud embeddings work immediately on Intel macOS. Explain
that local embeddings require a compatible system ONNX Runtime and give the supported command:

```bash
brew install onnxruntime
```

Do not imply that the Homebrew runtime is bundled or pinned by Chronacle.

- [ ] **Step 3: Update architecture and release-pipeline documentation**

Update the target matrix to distinguish packaged targets from runtime-library availability. Add
the five-entry native matrix, two-entry Flatpak matrix, Debian-to-Flatpak artifact flow, GNOME 50
sandbox contract, portal-only filesystem access, and final draft-publication gate. Record native
Windows ARM64, Flathub, signing/notarization, and updater support as deferred.

- [ ] **Step 4: Format and review documentation**

Run:

```bash
mise exec -- pnpm exec prettier --write README.md docs/user-guide.md docs/architecture.md
mise exec -- pnpm exec prettier --check README.md docs/user-guide.md docs/architecture.md
```

Expected: Prettier reports all three files formatted. Confirm searches for `Flathub`, `aarch64`,
`Intel`, and `Windows ARM64` describe the same support boundary in all three documents.

- [ ] **Step 5: Commit documentation**

```bash
git add README.md docs/user-guide.md docs/architecture.md
git commit -m "docs: explain multi-architecture release packages"
```

### Task 8: Run final release verification

**Files:**

- Verify only; modify the owning file and repeat the relevant task if a check fails.

- [ ] **Step 1: Run focused contracts**

```bash
scripts/ci/test-release-flatpak.sh
scripts/ci/test-pipeline.sh
cargo test -p Chronacle runtime_downloads -- --nocapture
```

Expected: all commands exit zero.

- [ ] **Step 2: Run the authoritative local pull-request gate**

```bash
scripts/ci/local-pr.sh
```

Expected: Backend quality, Frontend quality, and Acceptance tests all pass in the repository Docker
toolchain.

- [ ] **Step 3: Run release-only local checks**

```bash
cargo test -p Chronacle --features rocksdb
mise exec -- pnpm -C apps/desktop exec tauri build --no-bundle --features rocksdb
```

Expected: the RocksDB desktop suite and production build pass.

- [ ] **Step 4: Verify GitHub-only package jobs**

Confirm successful GitHub Actions runs for:

```text
Linux x86_64 native packages
Linux aarch64 native packages
x86_64 Flatpak package and startup smoke
aarch64 Flatpak package and startup smoke
macOS Apple Silicon package and startup smoke
macOS Intel package and startup smoke without bundled ONNX Runtime
Windows x86_64 MSI and NSIS packages
```

Confirm a deliberately failed test run leaves its GitHub release as a draft, then delete that test
draft and test tag through the GitHub UI or `gh` after resolving their exact names.

- [ ] **Step 5: Review the final diff and commits**

```bash
git status --short
git diff origin/main...HEAD --check
git log --oneline origin/main..HEAD
```

Expected: no uncommitted changes, no whitespace errors, and small logical commits matching the
tasks above. Record every command and GitHub Actions URL in the pull-request test report. Do not
open a pull request until `scripts/ci/local-pr.sh` has succeeded.
