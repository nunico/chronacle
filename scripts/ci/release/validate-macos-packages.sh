#!/usr/bin/env bash
set -euo pipefail

target=${RELEASE_TARGET:?RELEASE_TARGET is required}
shopt -s nullglob
root="target/${target}/release/bundle"
apps=("$root"/macos/*.app)
archives=("$root"/macos/*.app.tar.gz)
dmgs=("$root"/dmg/*.dmg)
[ "${#apps[@]}" -eq 1 ]
[ "${#archives[@]}" -eq 1 ]
[ "${#dmgs[@]}" -eq 1 ]
executable="${apps[0]}/Contents/MacOS/Chronacle"
[ -x "$executable" ]
case "$target" in
  aarch64-apple-darwin) file "$executable" | grep -q arm64 ;;
  x86_64-apple-darwin) file "$executable" | grep -q x86_64 ;;
esac
find "${apps[0]}/Contents/Resources" -path '*/pdfium/libpdfium.dylib' -type f | grep -q .
if [ "$target" = aarch64-apple-darwin ]; then
  find "${apps[0]}/Contents/Resources" -path '*/onnxruntime/libonnxruntime.dylib' -type f | grep -q .
else
  ! find "${apps[0]}/Contents/Resources" -path '*/onnxruntime/libonnxruntime.dylib' -type f | grep -q .
fi
"$executable" &
app_pid=$!
trap 'kill "$app_pid" 2>/dev/null || true; wait "$app_pid" 2>/dev/null || true' EXIT
for _ in {1..10}; do
  sleep 1
  kill -0 "$app_pid"
done
