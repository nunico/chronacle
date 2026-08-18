#!/usr/bin/env bash
set -euo pipefail

matching_releases=$(gh api --paginate --slurp \
  "repos/${GITHUB_REPOSITORY}/releases?per_page=100" | \
  jq 'add | map(select(.tag_name == env.GITHUB_REF_NAME))')
release_count=$(jq 'length' <<<"$matching_releases")
if [ "$release_count" -gt 1 ]; then
  echo "Multiple releases already use tag $GITHUB_REF_NAME" >&2
  exit 1
elif [ "$release_count" -eq 1 ]; then
  existing_draft=$(jq -r '.[0].draft' <<<"$matching_releases")
  if [ "$existing_draft" != true ]; then
    echo "Release $GITHUB_REF_NAME already exists and is not a draft" >&2
    exit 1
  fi
  release_id=$(jq -r '.[0].id' <<<"$matching_releases")
else
  created=$(gh api --method POST "repos/${GITHUB_REPOSITORY}/releases" \
    -f tag_name="$GITHUB_REF_NAME" \
    -f name="Chronacle $GITHUB_REF_NAME" \
    -F draft=true \
    -F prerelease=false)
  release_id=$(jq -r '.id' <<<"$created")
fi
[[ "$release_id" =~ ^[0-9]+$ ]]
echo "release_id=$release_id" >> "$GITHUB_OUTPUT"
