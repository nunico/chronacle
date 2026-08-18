#!/usr/bin/env bash
set -euo pipefail

version=$(jq -r '.version' apps/desktop/src-tauri/tauri.conf.json)
scripts/release-flatpak.sh artifacts/*.deb "$version" flatpak-out
