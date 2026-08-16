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

require_fixed "$manifest" "app-id: $app_id"
require_fixed "$manifest" 'runtime: org.gnome.Platform'
require_fixed "$manifest" 'runtime-version: "50"'
require_fixed "$manifest" 'sdk: org.gnome.Sdk'
require_fixed "$manifest" 'command: chronacle'
require_fixed "$manifest" 'buildsystem: simple'
require_fixed "$manifest" '--socket=wayland'
require_fixed "$manifest" '--socket=fallback-x11'
require_fixed "$manifest" '--share=ipc'
require_fixed "$manifest" '--device=dri'
require_fixed "$manifest" '--share=network'
reject_pattern "$manifest" '(^|[[:space:]])--filesystem=(home|host)(:|$|[[:space:]])'
reject_pattern "$manifest" '(^|[[:space:]])(only-arches|skip-arches):'
reject_pattern "$manifest" '(^|[[:space:]])(url|commit|tag):'

require_fixed "$manifest" 'path: Chronacle.deb'
require_fixed "$manifest" 'path: dev.tea-driven.chronacle.desktop.metainfo.xml'
[ "$(grep -Ec '^[[:space:]]+- type: file$' "$manifest")" -eq 2 ] ||
  fail "$manifest must define exactly two local file sources"
require_fixed "$manifest" 'ar x Chronacle.deb'
require_fixed "$manifest" 'tar -xzf data.tar.gz'
require_pattern "$manifest" 'test -x .*[Cc]hronacle'
require_pattern "$manifest" 'test -[ef] .*pdfium'
require_pattern "$manifest" 'test -[ef] .*onnxruntime'
require_pattern "$manifest" 'test -f .*\.desktop'
require_pattern "$manifest" 'test -f .*hicolor/32x32/apps.*\.png'
require_pattern "$manifest" 'test -f .*hicolor/128x128/apps.*\.png'
require_fixed "$manifest" '/app/bin/chronacle'
require_fixed "$manifest" '/app/lib/Chronacle'
require_fixed "$manifest" 'cp -a usr/lib/Chronacle /app/lib/Chronacle'
require_fixed "$manifest" "s/^Icon=.*/Icon=$app_id/"
require_fixed "$manifest" "$app_id.desktop"
require_pattern "$manifest" 'hicolor/.*/apps/'"$app_id"'\.png'
require_fixed "$manifest" "share/metainfo/$app_id.metainfo.xml"

require_fixed "$metainfo" '<component type="desktop-application">'
require_fixed "$metainfo" "<id>$app_id</id>"
require_fixed "$metainfo" '<metadata_license>CC0-1.0</metadata_license>'
require_fixed "$metainfo" '<project_license>AGPL-3.0-only</project_license>'
require_fixed "$metainfo" '<name>Chronacle</name>'
require_fixed "$metainfo" '<summary>'
require_fixed "$metainfo" '<description>'
require_fixed "$metainfo" '<developer id="dev.tea-driven">'
require_fixed "$metainfo" '<name>Tea Driven</name>'
require_fixed "$metainfo" '<url type="homepage">https://github.com/nunico/chronacle</url>'
require_fixed "$metainfo" '<url type="bugtracker">https://github.com/nunico/chronacle/issues</url>'
require_fixed "$metainfo" '<branding>'
require_fixed "$metainfo" '<color type="primary" scheme_preference="light">#3d5bff</color>'
require_fixed "$metainfo" '<color type="primary" scheme_preference="dark">#05060f</color>'
require_fixed "$metainfo" '<control>keyboard</control>'
require_fixed "$metainfo" '<control>pointing</control>'
require_fixed "$metainfo" '<content_rating type="oars-1.1" />'
require_fixed "$metainfo" "<launchable type=\"desktop-id\">$app_id.desktop</launchable>"
reject_pattern "$metainfo" '<screenshots([[:space:]>])'

echo 'Flatpak release contract passed.'
