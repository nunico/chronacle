#!/usr/bin/env bash
set -euo pipefail

target=${RELEASE_TARGET:?RELEASE_TARGET is required}
deb_arch=${DEB_ARCH:?DEB_ARCH is required}
flatpak_arch=${FLATPAK_ARCH:?FLATPAK_ARCH is required}
shopt -s nullglob
root="target/${target}/release/bundle"
debs=("$root"/deb/Chronacle_*.deb)
appimages=("$root"/appimage/Chronacle_*.AppImage)
rpms=("$root"/rpm/Chronacle-*.rpm)
if [ "${#debs[@]}" -ne 1 ]; then
  printf 'Expected exactly one release-ready Debian package, found %s\n' "${debs[@]}" >&2
  printf '  %s\n' "${debs[@]}" >&2
  exit 1
fi
if [ "${#appimages[@]}" -ne 1 ]; then
  printf 'Expected exactly one release-ready AppImage, found %s\n' "${appimages[@]}" >&2
  printf '  %s\n' "${appimages[@]}" >&2
  exit 1
fi
if [ "${#rpms[@]}" -ne 1 ]; then
  printf 'Expected exactly one release-ready RPM package, found %s\n' "${rpms[@]}" >&2
  printf '  %s\n' "${rpms[@]}" >&2
  exit 1
fi

extract_root=$(mktemp -d)
trap 'rm -rf -- "$extract_root"' EXIT
mkdir "$extract_root/deb" "$extract_root/rpm"
dpkg-deb -x "${debs[0]}" "$extract_root/deb"
rpm2cpio "${rpms[0]}" | (cd "$extract_root/rpm" && cpio -idm --quiet)
deb_executable="$extract_root/deb/usr/bin/chronacle"
rpm_executable="$extract_root/rpm/usr/bin/chronacle"
[ -x "$deb_executable" ]
[ -x "$rpm_executable" ]
[ -x "${appimages[0]}" ]

[ "$(dpkg-deb -f "${debs[0]}" Architecture)" = "$deb_arch" ]
case "$flatpak_arch" in
  x86_64)
    file "$deb_executable" | grep -Eq 'x86-64|x86_64'
    file "$rpm_executable" | grep -Eq 'x86-64|x86_64'
    file "${appimages[0]}" | grep -Eq 'x86-64|x86_64'
    [ "$(rpm -qp --qf '%{ARCH}' "${rpms[0]}")" = x86_64 ]
    ;;
  aarch64)
    file "$deb_executable" | grep -Eq 'ARM aarch64|aarch64'
    file "$rpm_executable" | grep -Eq 'ARM aarch64|aarch64'
    file "${appimages[0]}" | grep -Eq 'ARM aarch64|aarch64'
    [ "$(rpm -qp --qf '%{ARCH}' "${rpms[0]}")" = aarch64 ]
    ;;
esac
