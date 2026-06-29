#!/usr/bin/env bash
# Entrypoint for the E2E container: provide a DBus session + virtual X display,
# then run mocha. Optional args are passed to mocha as a spec override, e.g.
#   docker run --rm chronacle-e2e tests/e2e/ui/settings-toggle.e2e.mjs
set -euo pipefail

if [ "$#" -gt 0 ]; then
  set -- mocha --timeout 180000 "$@"
else
  set -- pnpm e2e:ui
fi

exec dbus-run-session -- xvfb-run -a --server-args="-screen 0 1280x1024x24" "$@"
