#!/usr/bin/env bash
set -euo pipefail

tag=${GITHUB_REF_NAME:?GITHUB_REF_NAME is required}
conf_version=$(node -p "require('./apps/desktop/src-tauri/tauri.conf.json').version")
if [ "v${conf_version}" != "${tag}" ]; then
  echo "Tag ${tag} does not match tauri.conf.json version ${conf_version}"
  exit 1
fi
echo "Tag ${tag} matches tauri.conf.json version ${conf_version}"
