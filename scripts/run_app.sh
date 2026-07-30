#!/usr/bin/env bash
# scripts/run_app.sh — Local dev runner for Cowri backend
# Usage: ./scripts/run_app.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/backend/.env"

for cmd in cargo curl openssl; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "ERROR: $cmd not found"; exit 1; }
done

# ── Bootstrap .env ────────────────────────────────────────────────────────────
if [ ! -f "$ENV_FILE" ]; then
  cp "$ROOT/backend/.env.example" "$ENV_FILE"
  JWT_SECRET=$(openssl rand -hex 32)
  sed -i "s/^JWT_SECRET=$/JWT_SECRET=$JWT_SECRET/" "$ENV_FILE"
  echo "✓ backend/.env created — fill in PAYSTACK_SECRET_KEY and DATABASE_URL"
fi

set -a; source "$ENV_FILE"; set +a

: "${JWT_SECRET:?JWT_SECRET must be set}"
: "${PAYSTACK_SECRET_KEY:?PAYSTACK_SECRET_KEY must be set}"
: "${DATABASE_URL:?DATABASE_URL must be set (e.g. postgres://user:pass@localhost/cowri)}"

export CORS_ORIGIN="${CORS_ORIGIN:-http://localhost:8080}"
export APP_URL="${APP_URL:-http://localhost:3000}"

# ── Build ─────────────────────────────────────────────────────────────────────
echo "Building..."
cd "$ROOT"
cargo build -p backend --quiet

echo ""
echo "🚀 Cowri API → http://localhost:3000"
echo "   DB          : $DATABASE_URL"
echo "   CORS_ORIGIN : $CORS_ORIGIN"
echo ""
exec "$ROOT/target/debug/backend"
