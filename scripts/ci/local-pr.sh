#!/bin/sh
set -eu

export DOCKER_BUILDKIT=1
docker build --progress=plain --target pr-gate -f Dockerfile.ci -t chronacle-pr-gate "$@" .
