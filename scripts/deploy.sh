#!/usr/bin/env bash
# Veritoken deployment script — Stellar Testnet
# Usage: bash scripts/deploy.sh [identity-name]
# Requires: stellar CLI, cargo, wasm32 target
#
# SECURITY: All asset tokens use Soroban constructors (__constructor) so that
# admin, kyc_registry, and compliance_engine are set atomically at deploy time.
# There is no window between deploy and initialization — front-running is not
# possible. Do NOT revert to a two-step deploy-then-initialize pattern.

set -euo pipefail

NETWORK="${STELLAR_NETWORK:-testnet}"
IDENTITY="${1:-alice}"
SOURCE="--source-account $IDENTITY --network $NETWORK"
ADMIN_ADDR="$(stellar keys address $IDENTITY)"

echo "==> Building all contracts..."
cargo build --release --target wasm32-unknown-unknown

WASM_DIR="target/wasm32-unknown-unknown/release"

build_wasm() {
  local name="$1"
  local wasm_path="$WASM_DIR/${name//-/_}.wasm"
  local size_before
  size_before=$(wc -c < "$wasm_path")
  echo "--- Optimizing $name.wasm (before: ${size_before} bytes)"
  stellar contract optimize --wasm "$wasm_path"
  local size_after
  size_after=$(wc -c < "$wasm_path")
  echo "    After: ${size_after} bytes (saved $((size_before - size_after)) bytes)"
}

build_wasm kyc_registry
build_wasm compliance_engine
build_wasm invoice_token
build_wasm property_token
build_wasm carbon_credit_token

echo ""
echo "==> Deploying KYC Registry..."
KYC_ID=$(stellar contract deploy \
  $SOURCE \
  --wasm "$WASM_DIR/kyc_registry.wasm" \
  -- \
  --admin "$ADMIN_ADDR")
echo "    KYC_REGISTRY_ID=$KYC_ID"

echo "==> Deploying Compliance Engine..."
CE_ID=$(stellar contract deploy \
  $SOURCE \
  --wasm "$WASM_DIR/compliance_engine.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID")
echo "    COMPLIANCE_ENGINE_ID=$CE_ID"

# Asset tokens pass all constructor args atomically — no separate initialize call needed or possible.
# The '--' separator passes arguments directly to the contract constructor (__constructor).

echo "==> Deploying Invoice Token..."
INV_ID=$(stellar contract deploy \
  $SOURCE \
  --wasm "$WASM_DIR/invoice_token.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID" \
  --compliance-engine "$CE_ID" \
  --meta '{"invoice_id":"PLACEHOLDER","issuer":"","debtor":"","face_value_usd":0,"discount_rate_bps":0,"due_date":0,"currency":"USD","ipfs_doc_hash":""}')
echo "    INVOICE_TOKEN_ID=$INV_ID"

echo "==> Deploying Property Token..."
PROP_ID=$(stellar contract deploy \
  $SOURCE \
  --wasm "$WASM_DIR/property_token.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID" \
  --compliance-engine "$CE_ID" \
  --meta '{"property_id":"PLACEHOLDER","legal_name":"","jurisdiction":"","address":"","total_valuation_usd":0,"total_shares":1000000,"property_type":"residential","ipfs_title_hash":"","kyc_tier_required":1}')
echo "    PROPERTY_TOKEN_ID=$PROP_ID"

echo "==> Deploying Carbon Credit Token..."
CARBON_ID=$(stellar contract deploy \
  $SOURCE \
  --wasm "$WASM_DIR/carbon_credit_token.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID" \
  --compliance-engine "$CE_ID" \
  --meta '{"project_id":"PLACEHOLDER","standard":"VCS","vintage_year":2024,"project_name":"","project_type":"forestry","country":"","verifier":"","ipfs_cert_hash":""}')
echo "    CARBON_TOKEN_ID=$CARBON_ID"

echo ""
echo "==> Writing .env to frontend..."
cat > frontend/.env <<EOF
VITE_STELLAR_NETWORK=$NETWORK
VITE_KYC_REGISTRY_ID=$KYC_ID
VITE_COMPLIANCE_ENGINE_ID=$CE_ID
VITE_INVOICE_TOKEN_ID=$INV_ID
VITE_PROPERTY_TOKEN_ID=$PROP_ID
VITE_CARBON_TOKEN_ID=$CARBON_ID
EOF

echo ""
echo "Done! Contract IDs written to frontend/.env"
echo "IMPORTANT: Update the placeholder --meta values above with real asset metadata before production deployment."
echo "Next: cd frontend && npm install && npm run dev"
