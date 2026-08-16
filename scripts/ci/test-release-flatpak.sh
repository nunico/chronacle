#!/bin/sh
set -eu

manifest=packaging/flatpak/dev.tea-driven.chronacle.desktop.yml
metainfo=packaging/flatpak/dev.tea-driven.chronacle.desktop.metainfo.xml
app_id=dev.tea-driven.chronacle.desktop

fail() {
  echo "Flatpak release contract failed: $1" >&2
  exit 1
}

require_file() {
  [ -f "$1" ] || fail "missing $1"
}

require_exact() {
  grep -Fxq -- "$2" "$1" || fail "$1 must contain the exact line: $2"
}

require_fixed() {
  grep -Fq -- "$2" "$1" || fail "$1 must contain: $2"
}

require_pattern() {
  grep -Eq -- "$2" "$1" || fail "$1 must match: $2"
}

reject_pattern() {
  if grep -Eq -- "$2" "$1"; then
    fail "$1 must not match: $2"
  fi
}

require_file "$manifest"
require_file "$metainfo"

require_exact "$manifest" "app-id: $app_id"
require_exact "$manifest" 'runtime: org.gnome.Platform'
require_exact "$manifest" 'runtime-version: "50"'
require_exact "$manifest" 'sdk: org.gnome.Sdk'
require_exact "$manifest" 'command: chronacle'
require_fixed "$manifest" 'buildsystem: simple'

finish_args_headers=$(awk '/^finish-args:[[:space:]]*$/ { count++ } END { print count + 0 }' "$manifest")
[ "$finish_args_headers" -eq 1 ] || fail "$manifest must define exactly one finish-args list"

expected_finish_args='--socket=wayland
--socket=fallback-x11
--share=ipc
--device=dri
--share=network'
actual_finish_args=$(
  awk '
    /^finish-args:[[:space:]]*$/ { inside = 1; next }
    inside && /^[^[:space:]#]/ { exit }
    inside && /^[[:space:]]*-[[:space:]]+/ {
      item = $0
      sub(/^[[:space:]]*-[[:space:]]+/, "", item)
      sub(/[[:space:]]+$/, "", item)
      print item
    }
  ' "$manifest"
)
[ "$actual_finish_args" = "$expected_finish_args" ] ||
  fail "$manifest finish-args must equal the five approved permissions"

reject_pattern "$manifest" '(^|[[:space:]])(only-arches|skip-arches):'
reject_pattern "$manifest" '(^|[[:space:]])(url|commit|tag):'

require_exact "$manifest" '        path: Chronacle.deb'
require_exact "$manifest" '        path: dev.tea-driven.chronacle.desktop.metainfo.xml'
[ "$(grep -Ec '^[[:space:]]+- type: file$' "$manifest")" -eq 2 ] ||
  fail "$manifest must define exactly two local file sources"
require_fixed "$manifest" 'ar x Chronacle.deb'
require_fixed "$manifest" 'tar -xzf data.tar.gz'
require_pattern "$manifest" 'test -x .*[Cc]hronacle'
require_exact "$manifest" '      - test -f usr/lib/Chronacle/resources/pdfium/libpdfium.so'
require_exact "$manifest" '      - test -f usr/lib/Chronacle/resources/onnxruntime/libonnxruntime.so'
require_exact "$manifest" '      - test -f usr/share/icons/hicolor/32x32/apps/chronacle.png'
require_exact "$manifest" '      - test -f usr/share/icons/hicolor/128x128/apps/chronacle.png'
require_pattern "$manifest" 'test -f .*\.desktop'
require_fixed "$manifest" '/app/bin/chronacle'
require_fixed "$manifest" '/app/lib/Chronacle'
require_fixed "$manifest" 'cp -a usr/lib/Chronacle /app/lib/Chronacle'
require_fixed "$manifest" "s/^Icon=.*/Icon=$app_id/"
require_fixed "$manifest" "$app_id.desktop"
require_pattern "$manifest" 'hicolor/.*/apps/'"$app_id"'\.png'
require_fixed "$manifest" "share/metainfo/$app_id.metainfo.xml"
require_exact "$manifest" '      - test -f /app/lib/Chronacle/resources/pdfium/libpdfium.so'
require_exact "$manifest" '      - test -f /app/lib/Chronacle/resources/onnxruntime/libonnxruntime.so'
require_exact "$manifest" "      - test -f /app/share/icons/hicolor/32x32/apps/$app_id.png"
require_exact "$manifest" "      - test -f /app/share/icons/hicolor/128x128/apps/$app_id.png"

first_install_line=$(grep -n '^[[:space:]]*- install ' "$manifest" | head -n 1 | cut -d: -f1)
for source_path in \
  usr/lib/Chronacle/resources/pdfium/libpdfium.so \
  usr/lib/Chronacle/resources/onnxruntime/libonnxruntime.so \
  usr/share/icons/hicolor/32x32/apps/chronacle.png \
  usr/share/icons/hicolor/128x128/apps/chronacle.png; do
  assertion_line=$(grep -nF -- "- test -f $source_path" "$manifest" | cut -d: -f1)
  [ "$assertion_line" -lt "$first_install_line" ] ||
    fail "$source_path must be asserted before installation begins"
done

require_fixed "$metainfo" '<component type="desktop-application">'
require_fixed "$metainfo" "<id>$app_id</id>"
require_fixed "$metainfo" '<metadata_license>CC0-1.0</metadata_license>'
require_fixed "$metainfo" '<project_license>AGPL-3.0-only</project_license>'
require_fixed "$metainfo" '<name>Chronacle</name>'
require_exact "$metainfo" '  <summary>A local-first TTRPG game-master assistant</summary>'
require_exact "$metainfo" '      Load rulebook PDFs, keep structured campaign notes, and ask questions with source citations.'
require_fixed "$metainfo" '<developer id="dev.tea-driven">'
require_fixed "$metainfo" '<name>Tea Driven</name>'
require_fixed "$metainfo" '<url type="homepage">https://github.com/nunico/chronacle</url>'
require_fixed "$metainfo" '<url type="bugtracker">https://github.com/nunico/chronacle/issues</url>'
require_fixed "$metainfo" '<branding>'
require_fixed "$metainfo" '<color type="primary" scheme_preference="light">#3d5bff</color>'
require_fixed "$metainfo" '<color type="primary" scheme_preference="dark">#05060f</color>'
require_fixed "$metainfo" '<control>keyboard</control>'
require_fixed "$metainfo" '<control>pointing</control>'
require_fixed "$metainfo" '<supports>'
reject_pattern "$metainfo" '<recommends>'
require_fixed "$metainfo" '<content_rating type="oars-1.1" />'
require_fixed "$metainfo" "<launchable type=\"desktop-id\">$app_id.desktop</launchable>"
reject_pattern "$metainfo" '<screenshots([[:space:]>])'

release_script=scripts/release-flatpak.sh
test_root=$(mktemp -d "${TMPDIR:-/tmp}/chronacle-flatpak-contract.XXXXXX")
cleanup_test() {
  rm -rf "$test_root"
}
trap cleanup_test 0
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

if "$release_script" >/dev/null 2>&1; then
  fail 'release-flatpak must reject missing arguments'
fi

touch "$test_root/not-a-deb.txt"
if "$release_script" "$test_root/not-a-deb.txt" 1.2.3 "$test_root/out" >/dev/null 2>&1; then
  fail 'release-flatpak must reject a non-Debian input'
fi

touch "$test_root/Chronacle.deb"
assert_version_rejected() {
  rejected_version=$1
  error_file="$test_root/version-error"
  if "$release_script" "$test_root/Chronacle.deb" "$rejected_version" \
    "$test_root/out" >"$error_file" 2>&1; then
    fail "release-flatpak must reject version $rejected_version"
  fi
  require_fixed "$error_file" 'version must use strict X.Y.Z syntax'
}

assert_version_rejected v1.2.3
assert_version_rejected 01.2.3
assert_version_rejected 1.02.3
assert_version_rejected 1.2.03

mkdir -p "$test_root/bin" "$test_root/state" "$test_root/output"
printf '%s\n' amd64 amd64 arm64 amd64 >"$test_root/state/deb-architectures"
touch "$test_root/output/caller-owned"

cat >"$test_root/bin/dpkg-deb" <<'STUB'
#!/bin/sh
set -eu
printf 'dpkg-deb %s\n' "$*" >>"$STUB_STATE/calls"
[ "$1" = -f ]
[ "$3" = Architecture ]
arch=$(sed -n '1p' "$STUB_STATE/deb-architectures")
[ -n "$arch" ]
sed '1d' "$STUB_STATE/deb-architectures" >"$STUB_STATE/deb-architectures.next"
mv "$STUB_STATE/deb-architectures.next" "$STUB_STATE/deb-architectures"
printf '%s\n' "$arch"
STUB

cat >"$test_root/bin/appstreamcli" <<'STUB'
#!/bin/sh
set -eu
printf 'appstreamcli %s\n' "$*" >>"$STUB_STATE/calls"
STUB

cat >"$test_root/bin/flatpak-builder" <<'STUB'
#!/bin/sh
set -eu
printf 'flatpak-builder %s\n' "$*" >>"$STUB_STATE/calls"
repo=
arch=
build_dir=
manifest=
for arg in "$@"; do
  case "$arg" in
    --repo=*) repo=${arg#--repo=} ;;
    --arch=*) arch=${arg#--arch=} ;;
    --*) ;;
    *)
      if [ -z "$build_dir" ]; then
        build_dir=$arg
      else
        manifest=$arg
      fi
      ;;
  esac
done
[ -n "$repo" ]
[ -n "$arch" ]
[ -f "$manifest" ]
context=$(dirname "$manifest")
[ -f "$context/Chronacle.deb" ]
[ -f "$context/dev.tea-driven.chronacle.desktop.metainfo.xml" ]
mkdir -p "$repo" "$build_dir"
printf '%s\n' "$arch" >"$STUB_STATE/flatpak-arch"
dirname "$repo" >"$STUB_STATE/temp-dir"
STUB

cat >"$test_root/bin/flatpak" <<'STUB'
#!/bin/sh
set -eu
printf 'flatpak %s\n' "$*" >>"$STUB_STATE/calls"

assert_temp_home() {
  [ "$HOME" != "$CALLER_HOME" ]
  case "$HOME" in
    */chronacle-flatpak.*/home) ;;
    *) echo "Flatpak received non-temporary HOME: $HOME" >&2; exit 1 ;;
  esac
  case "$XDG_DATA_HOME" in
    */chronacle-flatpak.*/xdg-data) ;;
    *) echo "Flatpak received non-temporary XDG_DATA_HOME: $XDG_DATA_HOME" >&2; exit 1 ;;
  esac
}

case "${1:-}" in
  build-bundle)
    shift
    while [ "${1#--}" != "$1" ]; do
      shift
    done
    repo=$1
    bundle=$2
    app_id=$3
    [ -d "$repo" ]
    [ "$app_id" = dev.tea-driven.chronacle.desktop ]
    mkdir -p "$(dirname "$bundle")"
    : >"$bundle"
    ;;
  --user)
    assert_temp_home
    [ "$2" = --noninteractive ]
    [ "$3" = install ]
    [ -f "$4" ]
    ;;
  info)
    assert_temp_home
    cat "$STUB_STATE/flatpak-arch"
    ;;
  kill)
    assert_temp_home
    [ "$2" = dev.tea-driven.chronacle.desktop ]
    touch "$STUB_STATE/flatpak-kill"
    if [ -f "$STUB_STATE/app-pid" ]; then
      kill -TERM "$(cat "$STUB_STATE/app-pid")" 2>/dev/null || :
    fi
    ;;
  run)
    assert_temp_home
    case " $* " in
      *' --command=sh '*)
        mkdir -p "$HOME/.var/app/dev.tea-driven.chronacle.desktop"
        printf '%s\n' "$HOME/.var/app/dev.tea-driven.chronacle.desktop" \
          >"$STUB_STATE/observed-app-home"
        exit 0
        ;;
      *)
        printf '%s\n' "$$" >"$STUB_STATE/app-pid"
        trap 'touch "$STUB_STATE/app-stopped"; exit 143' TERM
        trap 'touch "$STUB_STATE/app-stopped"; exit 130' INT
        while :; do /bin/sleep 1; done
        ;;
    esac
    ;;
  *)
    echo "unexpected flatpak invocation: $*" >&2
    exit 1
    ;;
esac
STUB

cat >"$test_root/bin/dbus-run-session" <<'STUB'
#!/bin/sh
set -eu
[ "$1" = -- ]
shift
"$@" &
child_pid=$!
stop_child() {
  kill -TERM "$child_pid" 2>/dev/null || :
  wait "$child_pid" 2>/dev/null || :
  touch "$STUB_STATE/dbus-reaped"
}
trap 'stop_child; exit 143' TERM
trap 'stop_child; exit 130' INT
if wait "$child_pid"; then status=0; else status=$?; fi
touch "$STUB_STATE/dbus-reaped"
exit "$status"
STUB

cat >"$test_root/bin/xvfb-run" <<'STUB'
#!/bin/sh
set -eu
[ "$1" = -a ]
shift
"$@" &
child_pid=$!
stop_child() {
  kill -TERM "$child_pid" 2>/dev/null || :
  wait "$child_pid" 2>/dev/null || :
  touch "$STUB_STATE/xvfb-reaped"
}
trap 'stop_child; exit 143' TERM
trap 'stop_child; exit 130' INT
if wait "$child_pid"; then status=0; else status=$?; fi
touch "$STUB_STATE/xvfb-reaped"
exit "$status"
STUB

cat >"$test_root/bin/sleep" <<'STUB'
#!/bin/sh
exec /bin/sleep "${STUB_SLEEP_SECONDS:-0.05}"
STUB

chmod +x "$test_root/bin/"*
export STUB_STATE="$test_root/state"
CALLER_HOME="$test_root/caller-home"
mkdir -p "$CALLER_HOME"
touch "$CALLER_HOME/caller-owned"
export CALLER_HOME
HOME=$CALLER_HOME
export HOME
original_path=$PATH
PATH="$test_root/bin:$PATH"
export PATH

"$release_script" "$test_root/Chronacle.deb" 0.0.0 "$test_root/output"
"$release_script" "$test_root/Chronacle.deb" 1.2.3 "$test_root/output"
"$release_script" "$test_root/Chronacle.deb" 1.2.3 "$test_root/output"

rm -f \
  "$test_root/state/app-pid" \
  "$test_root/state/app-stopped" \
  "$test_root/state/dbus-reaped" \
  "$test_root/state/flatpak-kill" \
  "$test_root/state/xvfb-reaped"
STUB_SLEEP_SECONDS=1
export STUB_SLEEP_SECONDS
"$release_script" "$test_root/Chronacle.deb" 2.3.4 "$test_root/output" &
release_pid=$!
signal_wait=0
while [ ! -f "$test_root/state/app-pid" ] && [ "$signal_wait" -lt 100 ]; do
  /bin/sleep 0.05
  signal_wait=$((signal_wait + 1))
done
require_file "$test_root/state/app-pid"
kill -TERM "$release_pid"
if wait "$release_pid"; then
  fail 'a terminated Flatpak release must fail'
else
  signal_status=$?
fi
[ "$signal_status" -eq 143 ] ||
  fail "a TERM-interrupted Flatpak release must exit 143, got $signal_status"
unset STUB_SLEEP_SECONDS

signal_temp_dir=$(cat "$test_root/state/temp-dir")
[ ! -e "$signal_temp_dir" ] || fail 'signal cleanup must remove the temporary build directory'
[ ! -e "$test_root/output/Chronacle_2.3.4_x86_64.flatpak" ] ||
  fail 'a terminated Flatpak release must not publish an output bundle'
require_file "$test_root/state/flatpak-kill"
require_file "$test_root/state/app-stopped"
require_file "$test_root/state/xvfb-reaped"
require_file "$test_root/state/dbus-reaped"

PATH=$original_path
export PATH

require_file "$test_root/output/Chronacle_0.0.0_x86_64.flatpak"
require_file "$test_root/output/Chronacle_1.2.3_x86_64.flatpak"
require_file "$test_root/output/Chronacle_1.2.3_aarch64.flatpak"
require_file "$test_root/output/caller-owned"
require_file "$CALLER_HOME/caller-owned"
[ ! -e "$CALLER_HOME/.var" ] || fail 'Flatpak smoke tests must not use the caller HOME'
require_pattern "$test_root/state/observed-app-home" '/chronacle-flatpak\.[^/]*/home/\.var/app/dev\.tea-driven\.chronacle\.desktop$'

require_fixed "$test_root/state/calls" 'appstreamcli validate --no-net'
require_fixed "$test_root/state/calls" 'flatpak-builder --force-clean --arch=x86_64 --repo='
require_fixed "$test_root/state/calls" 'flatpak-builder --force-clean --arch=aarch64 --repo='
require_fixed "$test_root/state/calls" 'flatpak build-bundle --arch=x86_64 --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo'
require_fixed "$test_root/state/calls" 'flatpak build-bundle --arch=aarch64 --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo'
require_fixed "$test_root/state/calls" 'flatpak --user --noninteractive install'
require_fixed "$test_root/state/calls" 'flatpak info --user --show-arch dev.tea-driven.chronacle.desktop'
require_fixed "$test_root/state/calls" '/app/bin/chronacle'
require_fixed "$test_root/state/calls" '/app/lib/Chronacle/resources/pdfium/libpdfium.so'
require_fixed "$test_root/state/calls" '/app/lib/Chronacle/resources/onnxruntime/libonnxruntime.so'
require_fixed "$test_root/state/calls" '/app/share/applications/dev.tea-driven.chronacle.desktop.desktop'
require_fixed "$test_root/state/calls" '/app/share/metainfo/dev.tea-driven.chronacle.desktop.metainfo.xml'
require_fixed "$test_root/state/calls" '/app/share/icons/hicolor/32x32/apps/dev.tea-driven.chronacle.desktop.png'
require_fixed "$test_root/state/calls" '/app/share/icons/hicolor/128x128/apps/dev.tea-driven.chronacle.desktop.png'
require_fixed "$test_root/state/calls" 'flatpak run dev.tea-driven.chronacle.desktop'
require_fixed "$test_root/state/calls" 'flatpak kill dev.tea-driven.chronacle.desktop'

echo 'Flatpak release contract passed.'
