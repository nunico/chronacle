# PR Build Pipeline Optimization Design

## Goal

Reduce repeated RocksDB and PDFium work during development and pull-request validation, split
acceptance testing from frontend quality checks, and provide a Docker command that reproduces the
complete pull-request gate locally.

## Current Problems

The workspace SurrealDB dependency enables `kv-rocksdb`, `kv-mem`, and SurrealDB's default
features for every crate. Cargo feature unification therefore pulls RocksDB, WebSocket support, and
TLS support into checks and tests that only use the in-memory engine. The desktop crate repeats the
same SurrealDB features and directly depends on `pdfium-render` even though PDF extraction belongs
to `chronacle-ingestion`.

The desktop build script also provisions PDFium and ONNX Runtime whenever it runs. Static checks
and memory-backed tests do not need those runtime resources, but they currently share the same
build path as a runnable desktop application.

The GitHub Actions `frontend-check` job mixes static frontend quality checks, Vitest, Playwright
browser installation, backend service E2E, and BDD acceptance. This prevents the independent work
from running in parallel and makes failures less specific. The repository's existing Docker image
only covers real-app UI E2E, not the pull-request gate.

## CI Job Structure

Pull-request CI will expose three independent jobs:

1. **Backend quality** (`backend-quality`): Rust formatting, Clippy, workspace tests, and
   `cargo deny check`.
2. **Frontend quality** (`frontend-quality`): Svelte/TypeScript typechecking, linting, and Vitest.
3. **Acceptance tests** (`acceptance`): backend Playwright E2E and generated BDD acceptance
   scenarios.

The two Node jobs will use identical Node and pnpm versions and the same pnpm-store cache key.
Each job will still perform a frozen install so its `node_modules` layout is correct, but downloaded
packages will come from the restored store. Only the acceptance job will install and cache the
Chromium browser and its system dependencies.

Main-only coverage and release-build jobs remain outside the local PR-gate scope. Real-app UI E2E
continues to use its existing dedicated workflow and Docker image.

## Rust Dependency Boundaries

The workspace SurrealDB declaration will disable default features and enable only `kv-mem`.
Library crates will inherit that lightweight configuration for their memory-backed tests.

RocksDB will be activated only by the desktop application path that opens a persistent local
database. The desktop crate will expose an explicit persistence feature, and normal Tauri
development and production commands will enable it. Persistence-specific tests will opt into it
explicitly; the ordinary workspace quality gate will remain memory-only.

The desktop crate will stop declaring `pdfium-render` directly because all PDFium API usage belongs
to `chronacle-ingestion`. PDFium support remains in the ingestion crate because it is real product
functionality, but resource downloads will only occur for runnable or distributable desktop builds.
Static checks and memory-backed tests will set explicit skip controls so the desktop build script
cannot fetch PDFium or ONNX Runtime.

The implementation must preserve these invariants:

- Production desktop builds open RocksDB-backed databases.
- In-memory service and integration tests continue to exercise real SurrealQL migrations and
  queries.
- Explicit persistence integration tests continue to cover RocksDB behavior.
- PDF extraction tests run when a PDFium library is available and retain their existing documented
  skip behavior otherwise.
- No new Rust dependency is introduced.

## Local Docker PR Gate

A dedicated CI Dockerfile will model the GitHub pull-request environment. It will install the
pinned Rust toolchain, Node/pnpm versions, native packages required by Tauri compilation, Chromium,
and the existing repository dependencies. It will not run real-app UI E2E, coverage, or a release
bundle.

A single repository command will build and run the image. Its stages will use the same names and
commands as GitHub Actions:

- `backend-quality`
- `frontend-quality`
- `acceptance`

BuildKit cache mounts will persist Cargo registry/git data, Rust build artifacts, the pnpm store,
and Playwright browser downloads across local runs. The source tree will be copied after stable
toolchain/dependency layers so source-only edits do not invalidate provisioning layers. Cache keys
and Docker inputs will include the owning manifests and lockfiles without manually editing or
printing lockfile contents.

The command exits non-zero as soon as any gate fails and prints which named gate failed. It is the
required local validation command before Chronacle pull requests are created.

## Workflow Parity

GitHub Actions and Docker will call repository-owned scripts rather than maintaining two separate
lists of commands. This makes command drift observable in review and keeps local validation aligned
with pull-request CI. GitHub retains job-level parallelism, while the local wrapper may execute
stages sequentially to provide deterministic output and maximize reuse of a single container cache.

The shared commands will use frozen dependency installation and the repository-pinned toolchain.
CI caching is an optimization only: a cold runner or clean Docker cache must still produce the same
result.

## Failure Handling

Dependency installation failures remain attributed to the named job or local stage. Docker will
not hide command output or convert failures into warnings. Native resource downloads are forbidden
in quality-only paths; an unexpected attempt is a pipeline defect rather than something the gate
silently retries.

Cache misses are valid and must fall back to a clean installation. Corrupted caches can be removed
without changing the correctness of any command.

## Verification

Implementation verification will include:

- Inspecting Cargo's resolved feature graph to prove RocksDB is absent from the ordinary workspace
  quality path and present in the desktop persistence path.
- Running the repository-owned backend, frontend, and acceptance commands directly.
- Building and running the Docker PR gate from a clean-enough cache state.
- Validating workflow syntax and checking that each GitHub job calls the same repository-owned
  command as Docker.
- Running `cargo deny check` as part of the backend-quality gate.

No user-visible application behavior changes, so this infrastructure change does not require a new
Gherkin feature scenario.
