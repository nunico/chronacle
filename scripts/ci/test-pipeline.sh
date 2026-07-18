#!/bin/sh
set -eu

fail() {
  printf 'pipeline contract failed: %s\n' "$1" >&2
  exit 1
}

rg -q '^surrealdb = \{ version = "2", default-features = false, features = \["kv-mem"\] \}$' Cargo.toml \
  || fail 'workspace SurrealDB must be memory-only'
rg -q '^scripts/ci/test-pipeline\.sh$' scripts/ci/backend-quality.sh \
  || fail 'backend quality must enforce the pipeline contract'
! rg -q '^pdfium-render =' apps/desktop/src-tauri/Cargo.toml \
  || fail 'desktop must not activate pdfium-render directly'

metadata_file=$(mktemp "${TMPDIR:-/tmp}/chronacle-metadata.XXXXXX")
trap 'rm -f "$metadata_file"' EXIT HUP INT TERM
cargo metadata --no-deps --format-version 1 >"$metadata_file" \
  || fail 'cargo metadata could not inspect workspace features'

if ! node - "$metadata_file" <<'NODE'
const fs = require('node:fs');

const metadata = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const surrealDependencies = metadata.packages.flatMap((pkg) =>
  pkg.dependencies
    .filter((dependency) => dependency.name === 'surrealdb')
    .map((dependency) => ({ dependency, manifest: pkg.manifest_path })),
);

if (surrealDependencies.length === 0) {
  throw new Error('workspace packages do not inherit SurrealDB');
}

for (const { dependency, manifest } of surrealDependencies) {
  if (dependency.uses_default_features) {
    throw new Error(`SurrealDB defaults are enabled by ${manifest}`);
  }
  if (dependency.features.includes('kv-rocksdb')) {
    throw new Error(`kv-rocksdb is activated directly by ${manifest}`);
  }
}

const desktop = metadata.packages.find((pkg) =>
  pkg.manifest_path.endsWith('/apps/desktop/src-tauri/Cargo.toml'),
);
if (!desktop) {
  throw new Error('desktop package is missing from cargo metadata');
}

const rocksdb = desktop.features.rocksdb;
if (!Array.isArray(rocksdb) || rocksdb.length !== 1 || rocksdb[0] !== 'surrealdb/kv-rocksdb') {
  throw new Error('desktop rocksdb feature must activate only surrealdb/kv-rocksdb');
}

for (const pkg of metadata.packages) {
  for (const [feature, activations] of Object.entries(pkg.features)) {
    if (
      activations.includes('surrealdb/kv-rocksdb') &&
      (pkg.manifest_path !== desktop.manifest_path || feature !== 'rocksdb')
    ) {
      throw new Error(`kv-rocksdb has an unintended feature path in ${pkg.manifest_path}`);
    }
  }
}

const defaults = [...(desktop.features.default || [])];
const reachable = new Set();
while (defaults.length > 0) {
  const feature = defaults.pop();
  if (reachable.has(feature)) continue;
  reachable.add(feature);
  for (const activation of desktop.features[feature] || []) {
    if (!activation.includes('/') && !activation.startsWith('dep:')) defaults.push(activation);
  }
}
if (reachable.has('rocksdb')) {
  throw new Error('desktop rocksdb feature must not be enabled by default');
}
NODE
then
  fail 'Cargo feature boundaries are invalid'
fi

workflow_job_runs_stage() {
  awk -v job="$1" -v script="scripts/ci/$1.sh" '
    $0 == "  " job ":" { in_job = 1; seen = 1; next }
    in_job && $0 ~ /^  [A-Za-z0-9_-]+:/ { in_job = 0 }
    in_job && $0 ~ /^[[:space:]]+run:/ {
      line = $0
      sub(/^[[:space:]]+run:[[:space:]]+/, "", line)
      sub(/[[:space:]]+$/, "", line)
      if (line == script) found = 1
    }
    END { exit !(seen && found) }
  ' .github/workflows/ci.yml
}

docker_stage_runs_stage() {
  awk -v stage="$1" -v script="scripts/ci/$1.sh" '
    $1 == "FROM" && $(NF - 1) == "AS" && $NF == stage {
      in_stage = 1
      seen = 1
      next
    }
    in_stage && $1 == "FROM" { in_stage = 0 }
    in_stage {
      line = $0
      sub(/^[[:space:]]+/, "", line)
      sub(/[[:space:]]+$/, "", line)
      if (line == "RUN " script || line == script) found = 1
    }
    END { exit !(seen && found) }
  ' Dockerfile.ci
}

for stage in backend-quality frontend-quality acceptance; do
  test -x "scripts/ci/$stage.sh" || fail "$stage entrypoint is missing"
  workflow_job_runs_stage "$stage" \
    || fail "$stage workflow does not use its shared entrypoint"
  docker_stage_runs_stage "$stage" \
    || fail "$stage Docker target does not use its shared entrypoint"
done

rg -q 'run: pnpm -C apps/desktop exec tauri build --no-bundle --features rocksdb' \
  .github/workflows/ci.yml \
  || fail 'main production build must build the desktop app with rocksdb'
rg -q 'run: pnpm exec tauri build --no-bundle --features rocksdb' \
  .github/workflows/e2e-ui.yml \
  || fail 'UI E2E workflow build must enable rocksdb'
rg -q '^RUN pnpm -C apps/desktop exec tauri build --no-bundle --features rocksdb$' \
  apps/desktop/tests/e2e/ui/Dockerfile \
  || fail 'UI E2E container build must enable rocksdb'

awk '
  /uses: tauri-apps\/tauri-action@/ { in_action = 1; seen = 1; next }
  in_action && /^[[:space:]]+with:/ { in_with = 1; next }
  in_action && in_with && /^[[:space:]]+args:[[:space:]]+--features rocksdb[[:space:]]*$/ {
    found = 1
  }
  END { exit !(seen && found) }
' .github/workflows/release.yml \
  || fail 'release packaging must enable rocksdb'
rg -q 'cargo test -p Chronacle --features rocksdb --test rocksdb_persistence' \
  .github/workflows/release.yml \
  || fail 'release validation must exercise RocksDB persistence'
