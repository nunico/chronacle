#!/bin/sh
set -eu

pnpm -C apps/desktop typecheck
pnpm -C apps/desktop lint
pnpm -C apps/desktop test:run
pnpm -C apps/website typecheck
pnpm -C apps/website lint
pnpm -C apps/website test:run
pnpm -C apps/website build
