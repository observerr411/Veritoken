# Mainnet Deployment Guide

This guide covers deploying Veritoken contracts to the Stellar mainnet in a production-ready, secure configuration. Follow every step in order. Do not skip the checklist.

---

## Pre-deployment Checklist

Complete every item before running any deployment command.

- [ ] All contracts compile cleanly: `cargo build --release --target wasm32-unknown-unknown`
- [ ] All tests pass: `cargo test --features testutils`
- [ ] `cargo clippy --target wasm32-unknown-unknown` reports no warnings
- [ ] Admin keypair is generated with a hardware wallet or secure HSM (never a hot key)
- [ ] Admin keypair has been backed up in at least two geographically separate locations
- [ ] You have a funded mainnet account with enough XLM to cover deployment fees (~10 XLM per contract, plus reserves)
- [ ] KYC verifier addresses have been collected and reviewed
- [ ] Compliance rules have been drafted and reviewed by legal counsel
- [ ] All `--meta` values (invoice details, property data, carbon credit standards) are finalised and accurate — **placeholders must not appear on mainnet**
- [ ] A deployment dry-run has been completed on testnet with the exact same commands
- [ ] An independent Soroban security review has been completed or scheduled

---

## Admin Key Management

The admin address controls minting, settlement, verifier management, and compliance rule updates. Compromise of this key is a critical security incident.

**Requirements:**

- Use a hardware wallet (Ledger) or an HSM for the admin keypair. Never use a hot key stored on a server.
- Create a dedicated mainnet identity in the Stellar CLI that references your hardware wallet:
  ```bash
  stellar keys add mainnet-admin --ledger
  ```
- Confirm the address:
  ```bash
  stellar keys address mainnet-admin
  ```
- Store the mnemonic / seed phrase offline in a fireproof safe. Never store it digitally.
- Consider a multi-signature scheme (e.g. 2-of-3 signers) for the admin account to eliminate single points of failure. Update the account's signing thresholds on-chain before deploying contracts.
- Rotate keys after any suspected compromise. The admin address is baked into each contract at deploy time; re-deploying all contracts is required after a key rotation.

---

## Deploying to Mainnet

> **Warning:** All `--meta` JSON values in the commands below are placeholders. Replace every field with real production data before running. Deploying placeholder metadata to mainnet cannot be undone.

Set up your environment:

```bash
export IDENTITY="mainnet-admin"
export NETWORK="mainnet"
export ADMIN_ADDR="$(stellar keys address $IDENTITY)"
```

**Build WASM artifacts:**

```bash
cargo build --release --target wasm32-unknown-unknown
WASM_DIR="target/wasm32-unknown-unknown/release"
```

**Optimize WASM (reduces deployment fees):**

```bash
stellar contract optimize --wasm "$WASM_DIR/kyc_registry.wasm"
stellar contract optimize --wasm "$WASM_DIR/compliance_engine.wasm"
stellar contract optimize --wasm "$WASM_DIR/invoice_token.wasm"
stellar contract optimize --wasm "$WASM_DIR/property_token.wasm"
stellar contract optimize --wasm "$WASM_DIR/carbon_credit_token.wasm"
```

**Deploy KYC Registry:**

```bash
KYC_ID=$(stellar contract deploy \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --wasm "$WASM_DIR/kyc_registry.wasm" \
  -- \
  --admin "$ADMIN_ADDR")
echo "KYC_REGISTRY_ID=$KYC_ID"
```

**Deploy Compliance Engine:**

```bash
CE_ID=$(stellar contract deploy \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --wasm "$WASM_DIR/compliance_engine.wasm" \
  -- \
  --admin "$ADMIN_ADDR")
echo "COMPLIANCE_ENGINE_ID=$CE_ID"
```

**Deploy Invoice Token** (replace `--meta` with real invoice data):

```bash
INV_ID=$(stellar contract deploy \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --wasm "$WASM_DIR/invoice_token.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID" \
  --compliance-engine "$CE_ID" \
  --meta '{"invoice_id":"INV-2024-001","issuer":"Acme Corp","debtor":"Globex LLC","face_value_usd":100000000000,"discount_rate_bps":250,"due_date":1900000000,"currency":"USD","ipfs_doc_hash":"QmYourRealHashHere"}')
echo "INVOICE_TOKEN_ID=$INV_ID"
```

**Deploy Property Token** (replace `--meta` with real property data):

```bash
PROP_ID=$(stellar contract deploy \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --wasm "$WASM_DIR/property_token.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID" \
  --compliance-engine "$CE_ID" \
  --meta '{"property_id":"PROP-001","legal_name":"123 Main St LLC","jurisdiction":"US-NY","address":"123 Main St, New York, NY 10001","total_valuation_usd":5000000,"total_shares":1000000,"property_type":"commercial","ipfs_title_hash":"QmYourTitleHashHere","kyc_tier_required":2}')
echo "PROPERTY_TOKEN_ID=$PROP_ID"
```

**Deploy Carbon Credit Token** (replace `--meta` with real project data):

```bash
CARBON_ID=$(stellar contract deploy \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --wasm "$WASM_DIR/carbon_credit_token.wasm" \
  -- \
  --admin "$ADMIN_ADDR" \
  --kyc-registry "$KYC_ID" \
  --compliance-engine "$CE_ID" \
  --meta '{"project_id":"VCS-2024-001","standard":"VCS","vintage_year":2024,"project_name":"Amazon Reforestation Initiative","project_type":"forestry","country":"BR","verifier":"Verra","ipfs_cert_hash":"QmYourCertHashHere"}')
echo "CARBON_TOKEN_ID=$CARBON_ID"
```

The Stellar mainnet RPC URL is `https://horizon.stellar.org` and the network passphrase is `Public Global Stellar Network ; September 2015`. The Stellar CLI resolves these automatically when `--network mainnet` is passed. See the [Stellar developer documentation](https://developers.stellar.org/docs/networks) for current network details.

To tag the deployment in git, run (after confirming all IDs are correct):
```bash
git tag v0.1.0
git push origin v0.1.0
```

---

## Configuring Compliance Rules

After deployment, configure the compliance engine with appropriate rules before onboarding any holders. All rule updates require the admin key.

```bash
stellar contract invoke \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --id "$CE_ID" \
  -- set_rules \
  --rules '{
    "max_transfer_amount": 0,
    "min_holding_period": 0,
    "max_holders": 2000,
    "require_same_jurisdiction": false,
    "paused": false
  }'
```

Rule fields:
- `max_transfer_amount` — maximum single-transfer amount in token units (0 = unlimited)
- `min_holding_period` — minimum seconds a holder must hold before transferring (0 = none)
- `max_holders` — maximum number of distinct holders (0 = unlimited); set to comply with Reg D / Reg S limits
- `require_same_jurisdiction` — restrict transfers to holders in the same jurisdiction
- `paused` — emergency pause; blocks all transfers when `true`

Consult legal counsel to determine the appropriate values for your asset type and jurisdiction.

---

## Onboarding KYC Verifiers

Verifiers are trusted addresses authorised to approve or revoke holder KYC status.

**Add a verifier:**

```bash
stellar contract invoke \
  --source-account "$IDENTITY" \
  --network "$NETWORK" \
  --id "$KYC_ID" \
  -- add_verifier \
  --verifier "G...VERIFIER_ADDRESS"
```

**Approve a holder (run by the verifier, not the admin):**

```bash
stellar contract invoke \
  --source-account "verifier-identity" \
  --network "$NETWORK" \
  --id "$KYC_ID" \
  -- approve \
  --verifier "G...VERIFIER_ADDRESS" \
  --addr "G...HOLDER_ADDRESS" \
  --tier 1 \
  --expiry 0 \
  --jurisdiction "US"
```

KYC tiers: `1` = Basic, `2` = Accredited, `3` = Institutional. Set `expiry` to a Unix timestamp to expire the approval, or `0` for no expiry.

**Revoke a holder:**

```bash
stellar contract invoke \
  --source-account "verifier-identity" \
  --network "$NETWORK" \
  --id "$KYC_ID" \
  -- revoke \
  --verifier "G...VERIFIER_ADDRESS" \
  --addr "G...HOLDER_ADDRESS"
```

---

## Post-deployment Verification

Run these checks immediately after deployment to confirm the contracts are correctly configured.

**Confirm contract IDs are live:**

```bash
stellar contract fetch --network mainnet --id "$KYC_ID"
stellar contract fetch --network mainnet --id "$CE_ID"
stellar contract fetch --network mainnet --id "$INV_ID"
```

**Verify admin address is correct:**

```bash
stellar contract invoke --network mainnet --id "$KYC_ID" -- admin
stellar contract invoke --network mainnet --id "$CE_ID" -- get_rules
```

**Verify invoice metadata:**

```bash
stellar contract invoke --network mainnet --id "$INV_ID" -- get_meta
stellar contract invoke --network mainnet --id "$INV_ID" -- is_settled
```

Expected: `is_settled` returns `false`; metadata fields match your deployment values.

**Verify compliance engine defaults (not paused, empty blocklist):**

```bash
stellar contract invoke --network mainnet --id "$CE_ID" -- get_rules
```

Expected: `paused: false`, `max_holders` matches your configuration.

Record all contract IDs in a secure internal document and update your frontend `.env` with the production values.

---

## Operational Runbook

### Add a new KYC verifier

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$KYC_ID" -- add_verifier --verifier "G...NEW_VERIFIER"
```

### Remove a KYC verifier

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$KYC_ID" -- remove_verifier --verifier "G...VERIFIER"
```

### Update compliance rules

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$CE_ID" -- set_rules --rules '{ ...updated rules... }'
```

### Emergency pause (block all transfers immediately)

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$CE_ID" -- pause
```

Verify the pause took effect:

```bash
stellar contract invoke --network mainnet --id "$CE_ID" -- get_rules
# paused should be true
```

### Unpause

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$CE_ID" -- unpause
```

### Blocklist a holder

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$CE_ID" -- add_to_blocklist --addr "G...HOLDER"
```

### Remove from blocklist

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$CE_ID" -- remove_from_blocklist --addr "G...HOLDER"
```

### Settle an invoice

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$INV_ID" -- settle
```

Once settled, transfers are blocked and holders may redeem. This action is **irreversible**.

### Issue tokens to a KYC-approved holder

```bash
stellar contract invoke \
  --source-account "$IDENTITY" --network mainnet \
  --id "$INV_ID" -- issue \
  --to "G...HOLDER" \
  --amount 1000000000
```

Amount is in token units at 7 decimal precision (1,000,000,000 = 100.0000000 tokens).
