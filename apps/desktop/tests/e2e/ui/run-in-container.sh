#!/usr/bin/env bash
# Entrypoint for the E2E container: provide a DBus session + virtual X display,
# then run mocha. Optional args are passed to mocha as a spec override, e.g.
#   docker run --rm chronacle-e2e tests/e2e/ui/settings-toggle.e2e.mjs
# Spec paths are relative to apps/desktop (the frontend package root).
set -euo pipefail

# The build context is the repo root (/app inside the container).
# Switch into the frontend package so pnpm resolves scripts and mocha from
# apps/desktop/node_modules, and so spec paths are relative to apps/desktop.
cd /app/apps/desktop

if [ "$#" -gt 0 ]; then
  set -- pnpm exec mocha --timeout 180000 "$@"
else
  set -- pnpm e2e:ui
fi

exec dbus-run-session -- xvfb-run -a --server-args="-screen 0 1280x1024x24" "$@"
