# PR Build Pipeline Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop ordinary PR validation from compiling RocksDB or provisioning desktop runtime libraries, split CI into clearly named parallel jobs, and add a cached Docker command that reproduces every PR gate locally.

**Architecture:** Repository-owned shell entrypoints define the three gates once and are called by GitHub Actions and Docker. Cargo defaults to a memory-only SurrealDB graph; the desktop `rocksdb` feature is enabled only by runnable Tauri and explicit persistence commands. A dedicated BuildKit Dockerfile provisions the shared Linux toolchain and caches Cargo, pnpm, and Playwright state.

**Tech Stack:** Cargo features, SurrealDB embedded engines, Tauri 2, GitHub Actions, pnpm 11, Playwright, Docker BuildKit, POSIX shell.

---

## File Map

- Modify `Cargo.toml`: make the workspace SurrealDB dependency memory-only with default features disabled.
- Modify `apps/desktop/src-tauri/Cargo.toml`: add the explicit `rocksdb` feature, remove duplicate PDFium activation, and require RocksDB for the runnable binary.
- Modify `apps/desktop/src-tauri/src/lib.rs`: compile persistent database startup only with the `rocksdb` feature while keeping memory-backed library tests available.
- Modify `apps/desktop/src-tauri/build.rs`: make runtime resource provisioning explicit and independently testable.
- Create `scripts/ci/backend-quality.sh`: canonical backend quality gate.
- Create `scripts/ci/frontend-quality.sh`: canonical frontend quality gate.
- Create `scripts/ci/acceptance.sh`: canonical backend E2E and BDD gate.
- Create `scripts/ci/test-pipeline.sh`: regression assertions for command parity, job split, and dependency boundaries.
- Create `scripts/ci/local-pr.sh`: build and run the local Docker PR gate.
- Create `Dockerfile.ci`: cached Linux environment and named PR-gate targets.
- Create `.dockerignore`: exclude build output, secrets, local state, and unrelated generated artifacts from the CI build context.
- Modify `.github/workflows/ci.yml`: expose `backend-quality`, `frontend-quality`, and `acceptance` jobs using shared scripts and shared dependency caches.
- Modify `mise.toml`: ensure Tauri development/build uses the RocksDB feature and expose the local PR task.
- Modify `README.md`, `AGENTS.md`, and `docs/architecture.md`: document optimized commands and the required local Docker pre-PR gate.

### Task 1: Add Pipeline Contract Tests

**Files:**

- Create: `scripts/ci/test-pipeline.sh`

- [ ] **Step 1: Write the failing configuration test**

Create an executable shell test that uses `rg` and `cargo metadata --no-deps --format-version 1` to assert:

```sh
#!/bin/sh
set -eu

fail() {
  printf 'pipeline contract failed: %s\n' "$1" >&2
  exit 1
}

rg -q '^surrealdb = \{ version = "2", default-features = false, features = \["kv-mem"\] \}$' Cargo.toml \
  || fail 'workspace SurrealDB must be memory-only'
rg -q '^rocksdb = \["surrealdb/kv-rocksdb"\]$' apps/desktop/src-tauri/Cargo.toml \
  || fail 'desktop RocksDB feature is missing'
! rg -q '^pdfium-render =' apps/desktop/src-tauri/Cargo.toml \
  || fail 'desktop must not activate pdfium-render directly'

for stage in backend-quality frontend-quality acceptance; do
  test -x "scripts/ci/$stage.sh" || fail "$stage entrypoint is missing"
  rg -q "scripts/ci/$stage.sh" .github/workflows/ci.yml \
    || fail "$stage workflow does not use its shared entrypoint"
  rg -q "scripts/ci/$stage.sh" Dockerfile.ci \
    || fail "$stage Docker target does not use its shared entrypoint"
done
```

- [ ] **Step 2: Run the test and verify it fails**

Run: `sh scripts/ci/test-pipeline.sh`

Expected: FAIL with `workspace SurrealDB must be memory-only`.

- [ ] **Step 3: Commit the red contract test**

```bash
git add scripts/ci/test-pipeline.sh
git commit -m "test: define PR pipeline contract"
```

### Task 2: Isolate RocksDB and Runtime Resource Provisioning

**Files:**

- Modify: `Cargo.toml`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/build.rs`
- Modify: `mise.toml`

- [ ] **Step 1: Make workspace SurrealDB memory-only**

Replace the workspace dependency with:

```toml
surrealdb = { version = "2", default-features = false, features = ["kv-mem"] }
```

- [ ] **Step 2: Define the desktop persistence boundary**

In the desktop manifest, add:

```toml
[features]
default = []
rocksdb = ["surrealdb/kv-rocksdb"]
```

Keep the direct `surrealdb.workspace = true` dependency because desktop command and test code uses
SurrealDB types directly. Remove only the direct `pdfium-render` line. Add
`required-features = ["rocksdb"]` to the `[[bin]]` target so a runnable desktop binary cannot be
built accidentally without persistence.

- [ ] **Step 3: Gate persistent startup in the library**

Apply `#[cfg(feature = "rocksdb")]` to the production database opener and the Tauri application
entrypoint that calls it. Keep memory-backed helpers and service modules available without the
feature. Add a compile-time diagnostic for any non-test application entrypoint that lacks the
feature.

- [ ] **Step 4: Separate resource policy from download mechanics**

Add a pure build-script helper:

```rust
fn should_download_runtime_resources() -> bool {
    env::var_os("CARGO_FEATURE_ROCKSDB").is_some()
        && env::var_os("CHRONACLE_SKIP_RUNTIME_DOWNLOADS").is_none()
}
```

Call `download_pdfium()` and `download_onnxruntime()` only when it returns true. Retain the existing
resource-specific skip variables for compatibility with explicit builds. Add unit tests for the
policy helper by moving the environment-independent decision to a function accepting booleans.

- [ ] **Step 5: Make Tauri commands opt into persistence**

Update `mise.toml` so development and production tasks call:

```toml
run = 'tauri dev --features rocksdb'
```

and:

```toml
run = 'node --run build -- --features rocksdb'
```

Verify the exact Tauri CLI forwarding syntax with `pnpm -C apps/desktop exec tauri build --help`
before finalizing the second command; use the CLI-documented placement if it differs.

- [ ] **Step 6: Verify the feature graphs**

Run:

```bash
CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1 cargo tree -e features -i surrealdb-rocksdb
```

Expected: Cargo reports that `surrealdb-rocksdb` is not present in the default workspace graph.

Run:

```bash
CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1 cargo tree -p Chronacle --features rocksdb -e features -i surrealdb-rocksdb
```

Expected: the path contains `Chronacle feature "rocksdb"`.

- [ ] **Step 7: Run memory and persistence tests**

Run:

```bash
CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1 cargo test --workspace
CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1 cargo test -p Chronacle --features rocksdb --test integration_test any_connect_opens_embedded_rocksdb -- --nocapture
```

Expected: both commands PASS; only the explicit second command compiles RocksDB.

- [ ] **Step 8: Commit the dependency boundary**

```bash
git add Cargo.toml apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/build.rs mise.toml
git commit -m "build: isolate native desktop dependencies"
```

### Task 3: Create Canonical Gate Entrypoints

**Files:**

- Create: `scripts/ci/backend-quality.sh`
- Create: `scripts/ci/frontend-quality.sh`
- Create: `scripts/ci/acceptance.sh`

- [ ] **Step 1: Implement the backend quality entrypoint**

Use strict shell mode and prevent runtime downloads:

```sh
#!/bin/sh
set -eu
export CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

Do not use `--all-features`; that flag would intentionally reactivate the RocksDB-only desktop
feature and defeat the lightweight gate.

- [ ] **Step 2: Implement the frontend quality entrypoint**

```sh
#!/bin/sh
set -eu

pnpm -C apps/desktop typecheck
pnpm -C apps/desktop lint
pnpm -C apps/desktop test:run
```

- [ ] **Step 3: Implement the acceptance entrypoint**

```sh
#!/bin/sh
set -eu

pnpm -C apps/desktop run e2e:backend
```

- [ ] **Step 4: Make scripts executable and rerun the contract test**

Run:

```bash
chmod +x scripts/ci/backend-quality.sh scripts/ci/frontend-quality.sh scripts/ci/acceptance.sh scripts/ci/test-pipeline.sh
sh scripts/ci/test-pipeline.sh
```

Expected: it progresses past entrypoint checks and fails because workflow/Docker references are not
implemented yet.

- [ ] **Step 5: Run each entrypoint directly**

Run:

```bash
scripts/ci/backend-quality.sh
scripts/ci/frontend-quality.sh
pnpm -C apps/desktop exec playwright install chromium
scripts/ci/acceptance.sh
```

Expected: all three PASS.

- [ ] **Step 6: Commit shared commands**

```bash
git add scripts/ci
git commit -m "build: add shared PR quality gates"
```

### Task 4: Split and Cache GitHub PR Jobs

**Files:**

- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Rename and simplify the Rust job**

Rename `rust-check` to `backend-quality`, set `name: Backend quality`, retain checkout, disk cleanup,
toolchain, Rust cache, and Linux native packages, then replace the individual commands and the
Docker-based cargo-deny action with:

```yaml
- name: Run backend quality gate
  run: scripts/ci/backend-quality.sh
```

- [ ] **Step 2: Restrict the frontend job to frontend quality**

Rename `frontend-check` to `frontend-quality`, set `name: Frontend quality`, and use
`actions/setup-node`'s pnpm cache:

```yaml
with:
  node-version: "22"
  cache: pnpm
  cache-dependency-path: pnpm-lock.yaml
```

After pnpm setup, run `pnpm install --frozen-lockfile --config.minimum-release-age=0`, followed by
`scripts/ci/frontend-quality.sh`. Remove Playwright installation and backend E2E from this job.

- [ ] **Step 3: Add the acceptance job**

Add `acceptance` with `name: Acceptance tests`, the same Node/pnpm cache configuration and frozen
install, then:

```yaml
- name: Cache Playwright Chromium
  uses: actions/cache@v5
  with:
    path: ~/.cache/ms-playwright
    key: ${{ runner.os }}-playwright-${{ hashFiles('pnpm-lock.yaml') }}

- name: Install Chromium
  working-directory: apps/desktop
  run: pnpm exec playwright install --with-deps chromium

- name: Run acceptance gate
  run: scripts/ci/acceptance.sh
```

- [ ] **Step 4: Update main-only dependencies**

Change `coverage` and `build` to require `[backend-quality, frontend-quality, acceptance]`. Keep
their main-only behavior unchanged.

- [ ] **Step 5: Validate workflow structure**

Run: `sh scripts/ci/test-pipeline.sh`

Expected: workflow checks pass; only Docker references remain failing.

- [ ] **Step 6: Commit the workflow split**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: split quality and acceptance jobs"
```

### Task 5: Add the Cached Local Docker PR Gate

**Files:**

- Create: `Dockerfile.ci`
- Create: `.dockerignore`
- Create: `scripts/ci/local-pr.sh`
- Modify: `mise.toml`

- [ ] **Step 1: Create a safe Docker context**

Exclude `.git`, `.env*`, `target`, all `node_modules`, frontend output, test reports, local model
caches, downloaded PDFium/ONNX libraries, and `graphify-out`. Do not exclude Cargo or pnpm
manifests.

- [ ] **Step 2: Build the reusable CI base**

Use Dockerfile syntax 1.7 and `rust:1.95-bookworm`. Install the WebKit/GTK packages used by GitHub,
Node 22, pnpm 11.5.1, Chromium dependencies, and `cargo-deny`. Copy dependency manifests before
source files where workspace layout permits, then copy the repository.

Use cache mounts such as:

```dockerfile
RUN --mount=type=cache,id=chronacle-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=chronacle-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=chronacle-target,target=/app/target \
    scripts/ci/backend-quality.sh
```

Use equivalent stable cache IDs for `/pnpm/store` and `/root/.cache/ms-playwright`.

- [ ] **Step 3: Add named Docker targets**

Create `backend-quality`, `frontend-quality`, and `acceptance` targets that execute the corresponding
repository scripts. Add a final `pr-gate` target depending on successful artifacts copied from all
three stages so `docker build --target pr-gate` proves every gate.

- [ ] **Step 4: Add the local wrapper**

Implement:

```sh
#!/bin/sh
set -eu

export DOCKER_BUILDKIT=1
docker build --progress=plain --target pr-gate -f Dockerfile.ci -t chronacle-pr-gate .
```

The wrapper must accept optional additional `docker build` arguments after the fixed safe arguments,
without using `eval`.

- [ ] **Step 5: Add a mise task**

Add:

```toml
[tasks.ci-pr]
description = 'Run the complete pull-request gate in Docker'
run = 'scripts/ci/local-pr.sh'
```

- [ ] **Step 6: Run the contract test**

Run: `sh scripts/ci/test-pipeline.sh`

Expected: PASS.

- [ ] **Step 7: Build and run the complete local gate**

Run: `scripts/ci/local-pr.sh`

Expected: the `pr-gate` target builds successfully after Backend quality, Frontend quality, and
Acceptance tests all pass. Re-run it and confirm dependency provisioning layers are cached.

- [ ] **Step 8: Commit Docker parity**

```bash
git add Dockerfile.ci .dockerignore scripts/ci/local-pr.sh mise.toml
git commit -m "ci: add cached local PR gate"
```

### Task 6: Update Developer and Architecture Guidance

**Files:**

- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `docs/architecture.md`

- [ ] **Step 1: Document named gate commands**

Replace duplicated command lists with the three repository scripts and explain that ordinary
backend quality is memory-only. Document the explicit RocksDB feature for Tauri development,
production builds, and persistence tests.

- [ ] **Step 2: Make Docker PR validation the required handoff**

Add this exact pre-PR command to README and AGENTS guidance:

```bash
scripts/ci/local-pr.sh
```

State that agents must run it successfully before creating a Chronacle pull request. Explain that it
covers PR gates only; coverage, release builds, and real-app UI E2E remain separate main/release
workflows.

- [ ] **Step 3: Update the architecture CI diagram**

Replace the old Rust/frontend two-job description with Backend quality, Frontend quality, and
Acceptance tests, including shared pnpm-store caching and repository-owned commands.

- [ ] **Step 4: Format and validate documentation**

Run:

```bash
pnpm -C apps/desktop exec prettier --write ../../README.md ../../AGENTS.md ../../docs/architecture.md
pnpm -C apps/desktop exec prettier --check ../../README.md ../../AGENTS.md ../../docs/architecture.md
```

Expected: formatting completes and the check reports all files formatted.

- [ ] **Step 5: Commit documentation**

```bash
git add README.md AGENTS.md docs/architecture.md
git commit -m "docs: require local Docker PR validation"
```

### Task 7: Final Verification and Memory Update

**Files:**

- Verify: all files above
- Create outside repository: `/Users/admin/.codex/memories/extensions/ad_hoc/notes/<timestamp>-chronacle-local-pr-gate.md`

- [ ] **Step 1: Run static pipeline contracts**

Run:

```bash
sh scripts/ci/test-pipeline.sh
git diff --check HEAD~5..HEAD
```

Expected: PASS with no whitespace errors.

- [ ] **Step 2: Prove native dependency isolation**

Run:

```bash
CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1 cargo tree -e features -i surrealdb-rocksdb
CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1 cargo tree -p Chronacle --features rocksdb -e features -i surrealdb-rocksdb
```

Expected: absent from default graph, present only through the explicit desktop feature.

- [ ] **Step 3: Run the authoritative PR gate**

Run: `scripts/ci/local-pr.sh`

Expected: Docker reports successful `backend-quality`, `frontend-quality`, `acceptance`, and final
`pr-gate` targets.

- [ ] **Step 4: Review repository state**

Run:

```bash
git status --short
git log --oneline -7
```

Expected: only the user's pre-existing untracked files remain; implementation commits are logically
grouped and no lockfile was manually edited.

- [ ] **Step 5: Record the requested durable rule**

Create one ad-hoc memory note stating: for Chronacle, run `scripts/ci/local-pr.sh` successfully before
creating any PR; it is the authoritative local reproduction of Backend quality, Frontend quality,
and Acceptance tests. Do not edit the memory registry directly.

- [ ] **Step 6: Perform final code review**

Use the repository's code-review and verification-before-completion skills. Resolve all blocking
findings, rerun affected gates, and report exact evidence without creating a PR.
