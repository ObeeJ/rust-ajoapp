#!/usr/bin/env bash
# scripts/run_app.sh — Local dev runner for Cowri backend
# Usage: ./scripts/run_app.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/backend/.env"

# ── 1. Check required tools ───────────────────────────────────────────────────
for cmd in cargo curl; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "ERROR: $cmd not found"; exit 1; }
done

# ── 2. Bootstrap .env if missing ─────────────────────────────────────────────
if [ ! -f "$ENV_FILE" ]; then
  echo "Creating backend/.env from example..."
  cp "$ROOT/backend/.env.example" "$ENV_FILE"

  # Generate a cryptographically random JWT_SECRET (32 bytes hex = 64 chars)
  JWT_SECRET=$(openssl rand -hex 32)
  sed -i "s/^JWT_SECRET=$/JWT_SECRET=$JWT_SECRET/" "$ENV_FILE"

  echo ""
  echo "⚠️  backend/.env created. Fill in PAYSTACK_SECRET_KEY before testing payments."
  echo "   JWT_SECRET has been auto-generated."
  echo ""
fi

# ── 3. Validate required env vars ────────────────────────────────────────────
source "$ENV_FILE"
: "${JWT_SECRET:?JWT_SECRET must be set in backend/.env}"
: "${PAYSTACK_SECRET_KEY:?PAYSTACK_SECRET_KEY must be set in backend/.env}"

export JWT_SECRET PAYSTACK_SECRET_KEY
export CORS_ORIGIN="${CORS_ORIGIN:-http://localhost:8080}"
export APP_URL="${APP_URL:-http://localhost:3000}"

# ── 4. Build ──────────────────────────────────────────────────────────────────
echo "Building backend..."
cd "$ROOT"
cargo build -p backend --quiet

# ── 5. Run ────────────────────────────────────────────────────────────────────
echo ""
echo "🚀 Starting Cowri API on http://localhost:3000"
echo "   CORS_ORIGIN : $CORS_ORIGIN"
echo "   APP_URL     : $APP_URL"
echo ""
exec "$ROOT/target/debug/backend"
