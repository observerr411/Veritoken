#!/usr/bin/env bash
# Veritoken deployment verification script
#
# Reads contract IDs from frontend/.env and makes a series of read calls to
# confirm that every deployed contract is live and correctly configured.
#
# Usage:
#   bash scripts/verify-deployment.sh [identity-name]
#
# The identity-name defaults to "alice" (same default as deploy.sh).
# It is used only as the --source-account for stellar contract invoke
# simulations; no transactions are submitted and no funds are spent.
#
# Exit codes:
#   0  — all checks passed ("Deployment verified.")
#   1  — one or more checks failed (specific error printed before exit)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$SCRIPT_DIR/../frontend/.env"

NETWORK="${STELLAR_NETWORK:-testnet}"
IDENTITY="${1:-alice}"
ADMIN_ADDR="$(stellar keys address "$IDENTITY")"

# ── Load .env ─────────────────────────────────────────────────────────────────

if [[ ! -f "$ENV_FILE" ]]; then
  echo "ERROR: $ENV_FILE not found. Run deploy.sh first." >&2
  exit 1
fi

# shellcheck source=/dev/null
source "$ENV_FILE"

KYC_ID="${VITE_KYC_REGISTRY_ID:-}"
CE_ID="${VITE_COMPLIANCE_ENGINE_ID:-}"
INV_ID="${VITE_INVOICE_TOKEN_ID:-}"
PROP_ID="${VITE_PROPERTY_TOKEN_ID:-}"
CARBON_ID="${VITE_CARBON_TOKEN_ID:-}"
RWA_ID="${VITE_RWA_TOKEN_ID:-}"

# ── Helpers ───────────────────────────────────────────────────────────────────

pass() { echo "  [ok] $1"; }
fail() { echo "ERROR: $1" >&2; exit 1; }

require_id() {
  local name="$1" id="$2"
  [[ -n "$id" ]] || fail "$name is empty in $ENV_FILE — run deploy.sh first"
}

invoke() {
  # invoke CONTRACT_ID FUNCTION [args...]
  local id="$1"; shift
  stellar contract invoke \
    --id "$id" \
    --source-account "$IDENTITY" \
    --network "$NETWORK" \
    -- "$@" 2>/dev/null
}

# ── Preflight: all required IDs must be present ───────────────────────────────

echo "==> Checking .env completeness..."
require_id "VITE_KYC_REGISTRY_ID"      "$KYC_ID"
require_id "VITE_COMPLIANCE_ENGINE_ID" "$CE_ID"
require_id "VITE_INVOICE_TOKEN_ID"     "$INV_ID"
require_id "VITE_PROPERTY_TOKEN_ID"    "$PROP_ID"
require_id "VITE_CARBON_TOKEN_ID"      "$CARBON_ID"
pass "All required IDs present"

# ── KYC Registry ─────────────────────────────────────────────────────────────

echo ""
echo "==> Verifying KYC Registry ($KYC_ID)..."
invoke "$KYC_ID" is_approved --addr "$ADMIN_ADDR" > /dev/null \
  || fail "KYC Registry is not responding (is_approved call failed)"
pass "KYC Registry is live"

VERIFIER_COUNT="$(invoke "$KYC_ID" verifier_count)"
[[ "$VERIFIER_COUNT" =~ ^[0-9]+$ ]] \
  || fail "KYC Registry verifier_count returned unexpected value: $VERIFIER_COUNT"
pass "KYC Registry verifier_count = $VERIFIER_COUNT"

# ── Compliance Engine ─────────────────────────────────────────────────────────

echo ""
echo "==> Verifying Compliance Engine ($CE_ID)..."
RULES="$(invoke "$CE_ID" get_rules)" \
  || fail "Compliance Engine is not responding (get_rules call failed)"
pass "Compliance Engine is live"

if echo "$RULES" | grep -q '"paused":true'; then
  fail "Compliance Engine is paused — transfers will be blocked"
fi
pass "Compliance Engine is not paused"

HOLDER_COUNT="$(invoke "$CE_ID" holder_count)"
[[ "$HOLDER_COUNT" =~ ^[0-9]+$ ]] \
  || fail "Compliance Engine holder_count returned unexpected value: $HOLDER_COUNT"
pass "Compliance Engine holder_count = $HOLDER_COUNT"

# ── Invoice Token ─────────────────────────────────────────────────────────────

echo ""
echo "==> Verifying Invoice Token ($INV_ID)..."
SUPPLY="$(invoke "$INV_ID" total_supply)" \
  || fail "Invoice Token is not responding (total_supply call failed)"
[[ "$SUPPLY" =~ ^[0-9]+$ ]] \
  || fail "Invoice Token total_supply returned unexpected value: $SUPPLY"
pass "Invoice Token is live (total_supply = $SUPPLY)"

# ── Property Token ────────────────────────────────────────────────────────────

echo ""
echo "==> Verifying Property Token ($PROP_ID)..."
SHARES="$(invoke "$PROP_ID" total_shares)" \
  || fail "Property Token is not responding (total_shares call failed)"
[[ "$SHARES" =~ ^[0-9]+$ ]] \
  || fail "Property Token total_shares returned unexpected value: $SHARES"
pass "Property Token is live (total_shares = $SHARES)"

# ── Carbon Credit Token ───────────────────────────────────────────────────────

echo ""
echo "==> Verifying Carbon Credit Token ($CARBON_ID)..."
CARBON_SUPPLY="$(invoke "$CARBON_ID" total_supply)" \
  || fail "Carbon Token is not responding (total_supply call failed)"
[[ "$CARBON_SUPPLY" =~ ^[0-9]+$ ]] \
  || fail "Carbon Token total_supply returned unexpected value: $CARBON_SUPPLY"
pass "Carbon Token is live (total_supply = $CARBON_SUPPLY)"

# ── RWA Token (optional) ──────────────────────────────────────────────────────

if [[ -n "$RWA_ID" ]]; then
  echo ""
  echo "==> Verifying RWA Token ($RWA_ID)..."
  ASSET_TYPE="$(invoke "$RWA_ID" asset_type)" \
    || fail "RWA Token is not responding (asset_type call failed)"
  [[ -n "$ASSET_TYPE" ]] \
    || fail "RWA Token asset_type returned an empty string"
  pass "RWA Token is live (asset_type = $ASSET_TYPE)"

  KYC_REG="$(invoke "$RWA_ID" kyc_registry)" \
    || fail "RWA Token kyc_registry call failed"
  [[ "$KYC_REG" == *"$KYC_ID"* ]] \
    || fail "RWA Token kyc_registry ($KYC_REG) does not match VITE_KYC_REGISTRY_ID"
  pass "RWA Token KYC registry matches deployed registry"

  CE_ADDR="$(invoke "$RWA_ID" compliance_engine)" \
    || fail "RWA Token compliance_engine call failed"
  [[ "$CE_ADDR" == *"$CE_ID"* ]] \
    || fail "RWA Token compliance_engine ($CE_ADDR) does not match VITE_COMPLIANCE_ENGINE_ID"
  pass "RWA Token compliance engine matches deployed engine"
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "Deployment verified."
