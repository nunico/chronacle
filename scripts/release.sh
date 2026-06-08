#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <version>"
  echo "  version  Semver without 'v' prefix, e.g. 0.2.0"
  exit 1
}

[[ $# -ne 1 ]] && usage
VERSION="$1"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: '$VERSION' is not a valid semver (expected X.Y.Z)"
  exit 1
fi

TAG="v${VERSION}"
ROOT="$(git rev-parse --show-toplevel)"
TAURI_CONF="$ROOT/src-tauri/tauri.conf.json"
CARGO_TOML="$ROOT/src-tauri/Cargo.toml"

# Require clean working tree
if ! git -C "$ROOT" diff --quiet || ! git -C "$ROOT" diff --cached --quiet; then
  echo "error: working tree has uncommitted changes — commit or stash them first"
  exit 1
fi

# Require tag doesn't already exist
if git -C "$ROOT" rev-parse "$TAG" &>/dev/null; then
  echo "error: tag $TAG already exists"
  exit 1
fi

echo "Bumping version to $VERSION..."

# tauri.conf.json
node -e "
  const fs = require('fs');
  const conf = JSON.parse(fs.readFileSync('$TAURI_CONF', 'utf8'));
  conf.version = '$VERSION';
  fs.writeFileSync('$TAURI_CONF', JSON.stringify(conf, null, '\t') + '\n');
"

# src-tauri/Cargo.toml — update the [package] version field only (first occurrence)
node -e "
  const fs = require('fs');
  const content = fs.readFileSync('$CARGO_TOML', 'utf8');
  fs.writeFileSync('$CARGO_TOML', content.replace(/^version = \"[^\"]+\"/m, 'version = \"$VERSION\"'));
"

git -C "$ROOT" add "$TAURI_CONF" "$CARGO_TOML"
git -C "$ROOT" commit -m "chore: bump version to $VERSION"
git -C "$ROOT" tag "$TAG"

echo "Created commit and tag $TAG"
echo ""
echo "Push with:"
echo "  git push && git push origin $TAG"
