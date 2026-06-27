<p align="center">
  <img src="./assets/logo.svg" alt="Veritoken" width="320"/>
</p>

<p align="center"><strong>RWA Tokenization Starter Kit for Stellar</strong></p>

Veritoken is a toolkit for bringing real-world assets on-chain. It gives any team a compliant, auditable foundation for tokenizing invoices, property shares, and carbon credits — with KYC verification and transfer compliance baked in at the protocol level, not bolted on after the fact.

The name fuses *veritas* (Latin: truth) with *token* — signalling verifiable, on-chain ownership of real things.

---

## The Problem

Tokenizing real-world assets on Stellar today means rebuilding the same compliance infrastructure from scratch on every project. Teams spend months writing KYC hooks, transfer restriction logic, and compliance metadata schemas before they can ship a single asset. The result is duplicated, inconsistently implemented code across the ecosystem — and slower time-to-market for every team that comes after.

## The Solution

Veritoken is a reusable, composable kit of Soroban contracts that any team can fork and deploy in days. The compliance layer is not an afterthought — it is the foundation everything else is built on. Every token transfer runs through an on-chain KYC registry and a configurable compliance engine before it executes.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Asset Token Layer                     │
│  invoice-token   property-token   carbon-credit-token   │
└────────────────────────┬────────────────────────────────┘
                         │ cross-contract calls
         ┌───────────────┴───────────────┐
         ▼                               ▼
┌─────────────────┐           ┌─────────────────────┐
│  KYC Registry   │           │  Compliance Engine   │
│                 │           │                      │
│ · Verifier mgmt │           │ · Transfer rules     │
│ · KYC tiers     │           │ · Blocklist          │
│ · Jurisdictions │           │ · Holding periods    │
│ · Expiry dates  │           │ · Pause / unpause    │
└─────────────────┘           └─────────────────────┘
```

### Contracts

| Contract | Description |
|---|---|
| `rwa-token` | Base SEP-41 token extended with RWA compliance hooks. Reusable for any asset type. |
| `kyc-registry` | On-chain KYC registry. Verifiers approve/revoke holders with tier (Basic / Accredited / Institutional) and jurisdiction metadata. |
| `compliance-engine` | Configurable transfer rules: max transfer size, minimum holding period, blocklist, emergency pause. |
| `invoice-token` | Tokenizes accounts-receivable invoices. Tracks face value, discount rate, due date, and IPFS document anchor. Settle-and-redeem lifecycle included. |
| `property-token` | Fractional real estate ownership. Includes a pro-rata dividend distribution mechanism with O(1) gas cost per holder. |
| `carbon-credit-token` | Issues verified carbon credits (1 token = 1 tonne CO₂e). Permanent on-chain retirement receipts with beneficiary metadata. |

### Compliance is enforced at the protocol level

Every transfer on every asset token makes two cross-contract calls before any balance changes:

1. `KycRegistry::is_approved(address)` — checks both sender and receiver have active, non-expired KYC
2. `ComplianceEngine::can_transfer(from, to, amount)` — enforces all configured rules

Neither call can be bypassed by the application layer.

---

## Quick Start

**Prerequisites:** Rust, `wasm32-unknown-unknown` target, Stellar CLI, Node.js ≥ 20

```bash
# Clone
git clone https://github.com/abore9769/Veritoken
cd Veritoken

# Create and fund a testnet identity
bash scripts/setup-identity.sh veritoken-dev

# Build all contracts and deploy to testnet
# (writes contract IDs to frontend/.env automatically)
bash scripts/deploy.sh veritoken-dev

# Start the frontend
cd frontend
npm install
npm run dev
```

### Build contracts only

```bash
cargo build --release --target wasm32-unknown-unknown
```

### Run tests

```bash
cargo test --features testutils
```

---

## Frontend

A React + Vite dashboard ships with the kit, wired to Freighter wallet and the Stellar SDK. It covers all five core workflows:

- **Dashboard** — overview of deployed asset types
- **Invoice** — form to tokenize an invoice with all compliance metadata
- **Property** — fractionalize real estate, view dividend state
- **Carbon Credits** — issue credits and submit retirements with on-chain receipts
- **KYC** — verifier interface to approve/revoke holders by tier and jurisdiction
- **Admin** — configure compliance rules and emergency pause

Copy `frontend/.env.example` to `frontend/.env` and fill in your deployed contract IDs to connect the UI to any network.

---

## Extending Veritoken

The kit is designed to be forked and customised:

- **New asset types** — extend `rwa-token` and implement asset-specific lifecycle logic
- **Custom compliance rules** — add new rule fields to `ComplianceRules` in `compliance-engine`
- **Multi-verifier KYC** — the verifier list in `kyc-registry` supports any number of approved verifiers
- **Off-chain anchoring** — every asset contract has an IPFS hash field for linking to legal documents

---

## Roadmap

- [x] Core contract suite — KYC registry, compliance engine, three asset templates
- [x] React frontend with Freighter wallet integration
- [x] CI pipeline (GitHub Actions) — fmt, clippy, tests, wasm build, frontend lint/build
- [x] Soroban test suite with simulated KYC and compliance scenarios
- [ ] SEP-41 compliance verification against the full standard
- [ ] Stellar CLI task runner for common admin operations
- [ ] Audit by an independent Soroban security reviewer
- [x] Mainnet deployment guide with production checklist — see [docs/mainnet-deployment.md](docs/mainnet-deployment.md)
- [ ] TypeScript SDK wrapping contract clients for frontend developers

---

## Repository Layout

```
Veritoken/
├── contracts/
│   ├── rwa-token/              # Base SEP-41 RWA token
│   ├── kyc-registry/           # On-chain KYC registry
│   ├── compliance-engine/      # Configurable transfer rules
│   ├── invoice-token/          # Invoice tokenization
│   ├── property-token/         # Fractional real estate
│   └── carbon-credit-token/    # Carbon credit lifecycle
├── frontend/                   # React + Vite + Freighter
│   └── src/
│       ├── lib/                # Stellar SDK + wallet bindings
│       ├── pages/              # One page per asset type
│       └── types/              # Shared TypeScript types
├── scripts/
│   ├── deploy.sh               # Build + deploy all contracts
│   └── setup-identity.sh       # Create and fund testnet identity
└── .github/workflows/ci.yml    # Build and type-check on every push
```

---

## Security

To report a vulnerability, please follow the [Security Policy](SECURITY.md). Do not open a public issue for security findings.

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for a full history of changes and the versioning policy.

---

## Contributing

Pull requests are welcome. For significant changes, please open an issue first to discuss the approach.

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/your-feature`)
3. Commit your changes
4. Open a pull request against `main`

Please ensure `cargo check --target wasm32-unknown-unknown` and `cargo test --features testutils` pass before submitting.

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

## About

Built for the Stellar ecosystem as public infrastructure. The goal is for any team building an RWA product on Stellar to be able to start from Veritoken rather than starting from zero.

> *"Making it infrastructure the whole Stellar ecosystem can build on."*
