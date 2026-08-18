#!/usr/bin/env bash
set -euo pipefail

release_id=${RELEASE_ID:?RELEASE_ID is required}
[[ "$release_id" =~ ^[0-9]+$ ]]
release=$(gh api "repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}")
release_tag=$(jq -r '.tag_name' <<<"$release")
release_draft=$(jq -r '.draft' <<<"$release")
if [ "$release_tag" != "$GITHUB_REF_NAME" ] || [ "$release_draft" != true ]; then
  echo "Release ID $RELEASE_ID is not the expected draft for $GITHUB_REF_NAME" >&2
  exit 1
fi
gh api --method PATCH \
  "repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}" \
  -F draft=false \
  -F prerelease=false
