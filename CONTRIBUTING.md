# Contributing to Veritoken

Thanks for your interest in contributing. Veritoken is a Soroban smart contract toolkit for RWA tokenization on Stellar — contributions of all kinds are welcome.

---

## Prerequisites

- Rust (stable)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)
- Node.js ≥ 20 (for frontend work)

---

## Setup

```bash
git clone https://github.com/abore9769/Veritoken
cd Veritoken
```

### Contracts

```bash
# Check all contracts compile
cargo check --target wasm32-unknown-unknown

# Run tests
cargo test --features testutils

# Build WASM artifacts
cargo build --release --target wasm32-unknown-unknown
```

### Frontend

```bash
cd frontend
cp .env.example .env   # fill in your deployed contract IDs
npm install
npm run dev
```

---

## Making Changes

1. Fork the repo and create a feature branch: `git checkout -b feat/your-feature`
2. Make your changes
3. Ensure the following pass before opening a PR:
   ```bash
   cargo fmt --check
   cargo clippy --target wasm32-unknown-unknown
   cargo test --features testutils
   ```
4. For frontend changes: `npm run build` and `npm run lint` must pass
5. Open a pull request against `main` with a clear description of what and why

---

## Good First Issues

Look for issues labelled [`good first issue`](https://github.com/abore9769/Veritoken/issues?q=label%3A%22good+first+issue%22) — these are scoped to be approachable without deep familiarity with the full codebase.

---

## Versioning

Veritoken uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Version numbers apply to the contract suite as a whole.

| Change type | Version bump |
|---|---|
| Breaking change to a public contract function (rename, remove, change signature) | **Major** (e.g. `0.1.0` → `1.0.0`) |
| New public function added to any contract | **Minor** (e.g. `0.1.0` → `0.2.0`) |
| Bug fix with no ABI change | **Patch** (e.g. `0.1.0` → `0.1.1`) |

When opening a PR that bumps the version:

1. Update the `version` field in every `contracts/*/Cargo.toml`
2. Add an entry to `CHANGELOG.md` under a new `[x.y.z]` section following the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format
3. After the PR merges, tag the commit: `git tag vx.y.z && git push origin vx.y.z`

Breaking changes (major bumps) must be discussed in an issue before implementation.

---

## Questions

Open an issue or start a discussion on GitHub. For significant changes, open an issue first to align on approach before writing code.
