#!/bin/sh
# Container start: optionally seed the demo baskets (idempotent), then run
# the given binary (default: api). Hosts set SEED_ON_START=true for demos.
#   start          -> api
#   start worker   -> background jobs
set -e
if [ "${SEED_ON_START:-false}" = "true" ]; then
  seed
fi
[ "$#" -eq 0 ] && set -- api
exec "$@"
