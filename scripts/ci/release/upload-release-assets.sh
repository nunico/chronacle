#!/usr/bin/env bash
set -euo pipefail

release_id=${RELEASE_ID:?RELEASE_ID is required}
[[ "$release_id" =~ ^[0-9]+$ ]]
assets=()
asset_names=()
while IFS= read -r -d '' asset; do
  asset_name=$(basename "$asset")
  case "$asset" in
    release-assets/native/chronacle-native-macos-arm64/*/Chronacle.app.tar.gz) asset_name=Chronacle_aarch64.app.tar.gz ;;
    release-assets/native/chronacle-native-macos-x86_64/*/Chronacle.app.tar.gz) asset_name=Chronacle_x64.app.tar.gz ;;
  esac
  assets+=("$asset")
  asset_names+=("$asset_name")
done < <(find release-assets/native release-assets/flatpak -type f \
  \( -name 'Chronacle_*.deb' -o -name 'Chronacle_*.AppImage' \
  -o -name 'Chronacle-*.rpm' \
  -o -name '*.app.tar.gz' -o -name '*.dmg' -o -name '*.msi' \
  -o -name '*.exe' -o -name '*.flatpak' \) -print0)

if [ "${#assets[@]}" -eq 0 ] || [ "${#assets[@]}" -ne "${#asset_names[@]}" ]; then
  echo "Release asset plan is empty or incomplete" >&2
  exit 1
fi
if [ "$(printf '%s\n' "${asset_names[@]}" | sort -u | wc -l)" -ne "${#asset_names[@]}" ]; then
  echo "Release asset names are not unique" >&2
  exit 1
fi

planned_names_json=$(printf '%s\n' "${asset_names[@]}" | jq -R . | jq -s .)
existing_assets=$(gh api --paginate --slurp "repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}/assets?per_page=100" | jq 'add')
while IFS= read -r stale_id; do
  [[ "$stale_id" =~ ^[0-9]+$ ]]
  gh api --method DELETE "repos/${GITHUB_REPOSITORY}/releases/assets/${stale_id}"
done < <(jq -r --argjson planned_names "$planned_names_json" '.[] | select(.name as $name | $planned_names | index($name) | not) | .id' <<<"$existing_assets")

for index in "${!assets[@]}"; do
  asset="${assets[$index]}"
  asset_name="${asset_names[$index]}"
  encoded_name=$(jq -rn --arg name "$asset_name" '$name | @uri')
  matching_assets=$(jq --arg name "$asset_name" 'map(select(.name == $name))' <<<"$existing_assets")
  matching_count=$(jq 'length' <<<"$matching_assets")
  if [ "$matching_count" -gt 1 ]; then
    echo "Multiple release assets are named $asset_name" >&2
    exit 1
  elif [ "$matching_count" -eq 1 ]; then
    existing_id=$(jq -r '.[0].id' <<<"$matching_assets")
    [[ "$existing_id" =~ ^[0-9]+$ ]]
    gh api --method DELETE "repos/${GITHUB_REPOSITORY}/releases/assets/${existing_id}"
  fi
  gh api --method POST "https://uploads.github.com/repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}/assets?name=${encoded_name}" \
    -H "Content-Type: application/octet-stream" \
    --input "$asset"
done
