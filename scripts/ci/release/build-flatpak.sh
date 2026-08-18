#!/usr/bin/env bash
set -euo pipefail

VERSION=$(jq -r '.version' apps/desktop/src-tauri/tauri.conf.json)
scripts/release-flatpak.sh artifacts/*.deb "$VERSION" flatpak-out
