#!/bin/sh
set -eu

app_id=dev.tea-driven.chronacle.desktop
runtime_repo=https://flathub.org/repo/flathub.flatpakrepo

fail() {
  echo "Flatpak release failed: $1" >&2
  exit 1
}

[ "$#" -eq 3 ] || fail 'usage: release-flatpak.sh <debian-package> <version> <output-directory>'

deb_path=$1
version=$2
output_dir=$3

[ -f "$deb_path" ] || fail "Debian package is not a regular file: $deb_path"
case "$deb_path" in
  *.deb) ;;
  *) fail "Debian package must have a .deb extension: $deb_path" ;;
esac

if ! printf '%s\n' "$version" |
  grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; then
  fail "version must use strict X.Y.Z syntax: $version"
fi

for tool in flatpak flatpak-builder appstreamcli dpkg-deb ar tar dbus-run-session xvfb-run; do
  command -v "$tool" >/dev/null 2>&1 || fail "required tool is unavailable: $tool"
done

repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || fail 'unable to resolve the repository root'
manifest="$repo_root/packaging/flatpak/$app_id.yml"
metainfo="$repo_root/packaging/flatpak/$app_id.metainfo.xml"
[ -f "$manifest" ] || fail "missing Flatpak manifest: $manifest"
[ -f "$metainfo" ] || fail "missing AppStream metadata: $metainfo"

deb_dir=$(dirname "$deb_path")
deb_name=$(basename "$deb_path")
deb_dir=$(cd "$deb_dir" && pwd -P) || fail "unable to resolve Debian package directory: $deb_dir"
deb_path="$deb_dir/$deb_name"

if [ -e "$output_dir" ] && [ ! -d "$output_dir" ]; then
  fail "output path is not a directory: $output_dir"
fi
mkdir -p "$output_dir" || fail "unable to create output directory: $output_dir"
output_dir=$(cd "$output_dir" && pwd -P) || fail "unable to resolve output directory: $output_dir"

deb_arch=$(dpkg-deb -f "$deb_path" Architecture) || fail 'unable to read Debian package architecture'
case "$deb_arch" in
  amd64) flatpak_arch=x86_64 ;;
  arm64) flatpak_arch=aarch64 ;;
  *) fail "unsupported Debian architecture: $deb_arch" ;;
esac

temp_dir=
smoke_pid=
smoke_active=0
cleanup_started=0

stop_smoke() {
  [ "$smoke_active" -eq 1 ] || return 0

  flatpak kill "$app_id" >/dev/null 2>&1 || :
  if [ -n "$smoke_pid" ] && kill -0 "$smoke_pid" 2>/dev/null; then
    kill -TERM "$smoke_pid" 2>/dev/null || :
    stop_attempt=0
    while kill -0 "$smoke_pid" 2>/dev/null && [ "$stop_attempt" -lt 5 ]; do
      sleep 1
      stop_attempt=$((stop_attempt + 1))
    done
    if kill -0 "$smoke_pid" 2>/dev/null; then
      kill -KILL "$smoke_pid" 2>/dev/null || :
    fi
  fi
  if [ -n "$smoke_pid" ]; then
    wait "$smoke_pid" 2>/dev/null || :
  fi
  smoke_active=0
}

cleanup() {
  [ "$cleanup_started" -eq 0 ] || return 0
  cleanup_started=1
  trap - HUP INT TERM
  stop_smoke
  if [ -n "$temp_dir" ] && [ -d "$temp_dir" ]; then
    rm -rf -- "$temp_dir"
  fi
}

handle_signal() {
  signal_status=$1
  trap - HUP INT TERM
  exit "$signal_status"
}

trap cleanup 0
trap 'handle_signal 129' HUP
trap 'handle_signal 130' INT
trap 'handle_signal 143' TERM
temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/chronacle-flatpak.XXXXXX") ||
  fail 'unable to create temporary build directory'

context="$temp_dir/context"
repo="$temp_dir/repo"
build_dir="$temp_dir/build"
bundle="$temp_dir/Chronacle_${version}_${flatpak_arch}.flatpak"
temp_home="$temp_dir/home"
xdg_data="$temp_dir/xdg-data"
mkdir -p "$context" "$temp_home" "$xdg_data"

cp "$manifest" "$context/$app_id.yml"
cp "$metainfo" "$context/$app_id.metainfo.xml"
cp "$deb_path" "$context/Chronacle.deb"

appstreamcli validate --no-net "$context/$app_id.metainfo.xml"
flatpak-builder --force-clean --arch="$flatpak_arch" --repo="$repo" \
  "$build_dir" "$context/$app_id.yml"
flatpak build-bundle --arch="$flatpak_arch" --runtime-repo="$runtime_repo" \
  "$repo" "$bundle" "$app_id"

HOME=$temp_home
XDG_DATA_HOME=$xdg_data
export HOME XDG_DATA_HOME
flatpak --user --noninteractive install "$bundle"

flatpak run --command=sh "$app_id" -c \
  'test -x /app/bin/chronacle &&
   test -f /app/lib/Chronacle/resources/pdfium/libpdfium.so &&
   test -f /app/lib/Chronacle/resources/onnxruntime/libonnxruntime.so &&
   test -f /app/share/applications/dev.tea-driven.chronacle.desktop.desktop &&
   test -f /app/share/metainfo/dev.tea-driven.chronacle.desktop.metainfo.xml &&
   test -f /app/share/icons/hicolor/32x32/apps/dev.tea-driven.chronacle.desktop.png &&
   test -f /app/share/icons/hicolor/128x128/apps/dev.tea-driven.chronacle.desktop.png'

installed_arch=$(flatpak info --user --show-arch "$app_id")
[ "$installed_arch" = "$flatpak_arch" ] ||
  fail "installed Flatpak architecture is $installed_arch, expected $flatpak_arch"

dbus-run-session -- xvfb-run -a flatpak run "$app_id" &
smoke_pid=$!
smoke_active=1
elapsed=0
while [ "$elapsed" -lt 10 ]; do
  sleep 1
  if ! kill -0 "$smoke_pid" 2>/dev/null; then
    if wait "$smoke_pid"; then
      app_status=0
    else
      app_status=$?
    fi
    fail "Flatpak exited before the ten-second startup smoke completed (status $app_status)"
  fi
  elapsed=$((elapsed + 1))
done

stop_smoke

mv "$bundle" "$output_dir/Chronacle_${version}_${flatpak_arch}.flatpak"
echo "Created $output_dir/Chronacle_${version}_${flatpak_arch}.flatpak"
