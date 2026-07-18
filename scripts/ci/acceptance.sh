#!/bin/sh
set -eu

pnpm -C apps/desktop run e2e:backend
