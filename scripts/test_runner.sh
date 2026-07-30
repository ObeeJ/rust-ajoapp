#!/usr/bin/env bash
# scripts/test_runner.sh — E2E + concurrency tests against live server
# Usage: ./scripts/test_runner.sh [BASE_URL]
# Requires: curl, jq, bc
set -euo pipefail

BASE="${1:-http://localhost:3000/v1}"
PASS=0; FAIL=0

# ── Helpers ───────────────────────────────────────────────────────────────────
green() { echo -e "\033[32m✓ $*\033[0m"; }
red()   { echo -e "\033[31m✗ $*\033[0m"; }

assert_eq() {
  local label="$1" got="$2" want="$3"
  if [ "$got" = "$want" ]; then
    green "$label"
    ((PASS++))
  else
    red "$label — got: $got, want: $want"
    ((FAIL++))
  fi
}

assert_contains() {
  local label="$1" haystack="$2" needle="$3"
  if echo "$haystack" | grep -q "$needle"; then
    green "$label"
    ((PASS++))
  else
    red "$label — '$needle' not found in: $haystack"
    ((FAIL++))
  fi
}

post() { curl -s -X POST -H "Content-Type: application/json" -c /tmp/cowri_cookies.txt -b /tmp/cowri_cookies.txt "$BASE$1" -d "$2"; }
get()  { curl -s -X GET  -b /tmp/cowri_cookies.txt "$BASE$1"; }

# ── Seed unique phones per run ────────────────────────────────────────────────
TS=$(date +%s)
PHONE_A="080${TS: -8}"
PHONE_B="081${TS: -8}"

echo ""
echo "═══════════════════════════════════════════════"
echo " Cowri E2E Test Runner  →  $BASE"
echo "═══════════════════════════════════════════════"
echo ""

# ── 1. Register ───────────────────────────────────────────────────────────────
echo "── Auth ──────────────────────────────────────"
rm -f /tmp/cowri_cookies.txt

REG=$(post "/auth/register" "{\"name\":\"Alice\",\"phone\":\"$PHONE_A\",\"pin\":\"1234\"}")
assert_contains "Register returns user"   "$REG" "\"name\""
assert_contains "Register returns wallet" "$REG" "\"available_kobo\""

# ── 2. Duplicate phone rejected ───────────────────────────────────────────────
DUP=$(post "/auth/register" "{\"name\":\"Alice2\",\"phone\":\"$PHONE_A\",\"pin\":\"5678\"}")
STATUS=$(echo "$DUP" | jq -r '.error // empty')
assert_contains "Duplicate phone → error" "$STATUS" "already registered"

# ── 3. Login ──────────────────────────────────────────────────────────────────
rm -f /tmp/cowri_cookies.txt
LOGIN=$(post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"1234\"}")
assert_contains "Login returns user" "$LOGIN" "\"name\""

# ── 4. Wrong PIN ──────────────────────────────────────────────────────────────
rm -f /tmp/cowri_cookies.txt
BAD=$(post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"9999\"}")
assert_contains "Wrong PIN → error" "$BAD" "Invalid"

# Re-login correctly for remaining tests
rm -f /tmp/cowri_cookies.txt
post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"1234\"}" > /dev/null

# ── 5. Wallet ─────────────────────────────────────────────────────────────────
echo ""
echo "── Wallet ────────────────────────────────────"
WALLET=$(get "/wallet")
BAL=$(echo "$WALLET" | jq -r '.available_kobo')
assert_eq "Initial balance is 0" "$BAL" "0"

# ── 6. Idempotency on fund_wallet ─────────────────────────────────────────────
IDEM_KEY="idem-test-$(date +%s%N)"
FUND1=$(curl -s -X POST -H "Content-Type: application/json" \
  -H "X-Idempotency-Key: $IDEM_KEY" \
  -b /tmp/cowri_cookies.txt -c /tmp/cowri_cookies.txt \
  "$BASE/wallet/fund" \
  -d "{\"amount_kobo\":50000,\"email\":\"alice@test.com\"}")
FUND2=$(curl -s -X POST -H "Content-Type: application/json" \
  -H "X-Idempotency-Key: $IDEM_KEY" \
  -b /tmp/cowri_cookies.txt -c /tmp/cowri_cookies.txt \
  "$BASE/wallet/fund" \
  -d "{\"amount_kobo\":50000,\"email\":\"alice@test.com\"}")

REF1=$(echo "$FUND1" | jq -r '.reference // empty')
REF2=$(echo "$FUND2" | jq -r '.reference // empty')
assert_eq "Idempotent fund — same reference returned" "$REF1" "$REF2"

# ── 7. Ajo ────────────────────────────────────────────────────────────────────
echo ""
echo "── Ajo ───────────────────────────────────────"

# Register second user
rm -f /tmp/cowri_cookies_b.txt
post "/auth/register" "{\"name\":\"Bob\",\"phone\":\"$PHONE_B\",\"pin\":\"5678\"}" > /dev/null

# Back to Alice
rm -f /tmp/cowri_cookies.txt
post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"1234\"}" > /dev/null

GROUP=$(post "/ajo" "{\"name\":\"Test Ajo\",\"contribution_kobo\":10000,\"frequency\":\"monthly\",\"member_count\":2}")
GROUP_ID=$(echo "$GROUP" | jq -r '.id // empty')
assert_contains "Create ajo group" "$GROUP_ID" "-"

# Join as Bob
rm -f /tmp/cowri_cookies.txt
post "/auth/login" "{\"phone\":\"$PHONE_B\",\"pin\":\"5678\"}" > /dev/null
JOIN=$(post "/ajo/$GROUP_ID/join" "{}")
assert_contains "Bob joins group" "$JOIN" "\"group_id\""

# Duplicate join rejected
DUP_JOIN=$(post "/ajo/$GROUP_ID/join" "{}")
assert_contains "Duplicate join → 409" "$DUP_JOIN" "Already"

# ── 8. Bills ──────────────────────────────────────────────────────────────────
echo ""
echo "── Bills ─────────────────────────────────────"
rm -f /tmp/cowri_cookies.txt
post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"1234\"}" > /dev/null

BILL=$(post "/bills" "{\"title\":\"Dinner\",\"total_kobo\":20000,\"participant_phones\":[\"$PHONE_B\"]}")
BILL_ID=$(echo "$BILL" | jq -r '.id // empty')
assert_contains "Create bill" "$BILL_ID" "-"

# ── 9. Rate limiting ──────────────────────────────────────────────────────────
echo ""
echo "── Rate Limiting ─────────────────────────────"
rm -f /tmp/cowri_cookies.txt
for i in $(seq 1 5); do
  post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"0000\"}" > /dev/null
done
LOCKED=$(post "/auth/login" "{\"phone\":\"$PHONE_A\",\"pin\":\"1234\"}")
assert_contains "Account locked after 5 failures" "$LOCKED" "Too many"

# ── 10. Concurrency stress — 10 parallel debits ───────────────────────────────
echo ""
echo "── Concurrency (unit tests) ──────────────────"
cd "$(dirname "$0")/.."
CONC=$(JWT_SECRET="test-secret-that-is-long-enough-32b" \
       PAYSTACK_SECRET_KEY="sk_test_x" \
       cargo test -p backend concurrent_debits_never_overdraft --quiet 2>&1)
if echo "$CONC" | grep -q "test result: ok"; then
  green "Concurrency: 10 parallel debits — no overdraft, ledger invariant holds"
  ((PASS++))
else
  red "Concurrency test failed"
  echo "$CONC"
  ((FAIL++))
fi

# ── 11. Ledger invariant (unit) ───────────────────────────────────────────────
INV=$(JWT_SECRET="test-secret-that-is-long-enough-32b" \
      PAYSTACK_SECRET_KEY="sk_test_x" \
      cargo test -p backend ledger_invariant --quiet 2>&1)
if echo "$INV" | grep -q "test result: ok"; then
  green "Ledger invariant: sum(credits) - sum(debits) == balance"
  ((PASS++))
else
  red "Ledger invariant test failed"
  ((FAIL++))
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════"
echo " Results: $PASS passed, $FAIL failed"
echo "═══════════════════════════════════════════════"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
