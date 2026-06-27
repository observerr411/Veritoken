# Changelog

All notable changes to Veritoken will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

For the versioning policy that governs when each version number is bumped, see the [Versioning](CONTRIBUTING.md#versioning) section in CONTRIBUTING.md.

---

## [Unreleased]

### Added
- `test_transfer_blocked_after_settlement` and `test_transfer_from_blocked_after_settlement` tests for invoice-token lifecycle correctness
- `docs/mainnet-deployment.md` — production deployment guide with pre-deployment checklist, admin key management, deployment command sequence, compliance configuration, KYC verifier onboarding, post-deployment verification, and operational runbook
- `SECURITY.md` — vulnerability disclosure policy with response timeline and safe harbour language
- `CHANGELOG.md` and versioning strategy documented in CONTRIBUTING.md

---

## [0.1.0] — 2026-06-26

Initial public release of the Veritoken RWA Tokenization Starter Kit for Stellar.

### Added

**Core contracts**

- `kyc-registry` — On-chain KYC registry. Verifiers approve or revoke holders with tier (Basic / Accredited / Institutional) and jurisdiction metadata. Supports multiple verifiers and optional expiry timestamps.
- `compliance-engine` — Configurable transfer rules: maximum transfer size, minimum holding period, maximum holder count, same-jurisdiction restriction, and emergency pause. All rules enforced atomically per transfer.
- `rwa-token` — Base SEP-41 token extended with RWA compliance hooks. Every transfer calls `kyc-registry::is_approved` and `compliance-engine::can_transfer` before executing. Reusable as the foundation for any asset type.

**Asset token templates**

- `invoice-token` — Tokenizes accounts-receivable invoices. Stores face value, discount rate, due date, IPFS document hash, issuer, and debtor. Includes a settle-and-redeem lifecycle: minting is blocked post-settlement; redemption (burn) is enabled post-settlement and subject to compliance checks.
- `property-token` — Fractional real estate ownership. Stores property identifier, legal name, jurisdiction, valuation, total shares, and IPFS title hash. Includes a pro-rata dividend distribution mechanism with O(1) gas cost per holder.
- `carbon-credit-token` — Issues verified carbon credits (1 token = 1 tonne CO₂e). Stores project identifier, standard (VCS, Gold Standard, etc.), vintage year, and IPFS certificate hash. Permanent on-chain retirement receipts with beneficiary metadata.

**Infrastructure**

- React + Vite frontend dashboard wired to Freighter wallet. Covers all five workflows: Invoice, Property, Carbon Credits, KYC, and Admin.
- `scripts/deploy.sh` — Build, optimise, and deploy all contracts to Stellar testnet in a single command. Writes contract IDs to `frontend/.env`.
- `scripts/setup-identity.sh` — Create and fund a testnet identity.
- GitHub Actions CI pipeline: `cargo fmt`, `cargo clippy`, `cargo test --features testutils`, WASM build, frontend lint and build.
- Soroban test suite covering KYC enforcement, compliance rule scenarios, invoice lifecycle, property dividend distribution, and carbon credit retirement.

[Unreleased]: https://github.com/abore9769/Veritoken/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/abore9769/Veritoken/releases/tag/v0.1.0
