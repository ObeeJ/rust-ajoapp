#!/usr/bin/env bash
# scripts/run_app.sh — Local dev runner (no Docker)
# Usage: ./scripts/run_app.sh
# Requires: cargo, Postgres running on localhost:5432
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/backend/.env"

[ -f "$ENV_FILE" ] || { echo "ERROR: backend/.env not found. Copy backend/.env.example and fill it in."; exit 1; }

# Load env — swap Docker hostnames for localhost
set -a
source "$ENV_FILE"
set +a

# Override DB/Redis hosts for local (non-Docker) run
export DATABASE_URL="${DATABASE_URL//@db:/@localhost:}"
export REDIS_URL="${REDIS_URL//redis:6379/localhost:6379}"

: "${JWT_SECRET:?JWT_SECRET must be set in backend/.env}"
: "${PAYSTACK_SECRET_KEY:?PAYSTACK_SECRET_KEY must be set in backend/.env}"
: "${DATABASE_URL:?DATABASE_URL must be set}"

echo "Building..."
cd "$ROOT"
cargo build -p backend --quiet

echo ""
echo "🚀 Cowri API → http://localhost:3000"
echo "   DB  : $DATABASE_URL"
echo "   CORS: $CORS_ORIGIN"
echo ""
exec "$ROOT/target/debug/backend"
