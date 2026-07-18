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
