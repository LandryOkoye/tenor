#!/usr/bin/env bash
# One command for a free, offline dev backend (no real money, no keys):
# Postgres in Docker + the API with mock Panta + seeded curated baskets.
#
#   ./scripts/dev-mock.sh            # API on http://localhost:8080 (Ctrl-C to stop)
#
# The dev admin wallet (fixed keypair, no funds) is
# GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
docker compose -f "$ROOT/infra/docker-compose.yml" up -d postgres >/dev/null
until docker compose -f "$ROOT/infra/docker-compose.yml" exec -T postgres pg_isready -U app >/dev/null 2>&1; do sleep 1; done

export APP_ENV=development PANTA_MOCK=true
export DATABASE_URL=postgres://app:app@localhost:5432/app
export ADMIN_WALLETS=GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB
export FEE_WALLET_PUBKEY=1FeeWa11et111111111111111111111111111111111
export API_BIND=${API_BIND:-0.0.0.0:8080}

cd "$ROOT/backend"
cargo build -q --bin api --example seed
./target/debug/api &
API=$!
trap 'kill $API 2>/dev/null' EXIT
until curl -sf "http://localhost:${API_BIND##*:}/healthz" >/dev/null; do sleep 1; done
SEED_API="http://localhost:${API_BIND##*:}" ./target/debug/examples/seed
echo "API ready on http://localhost:${API_BIND##*:} (mock Panta). Ctrl-C to stop."
wait $API
