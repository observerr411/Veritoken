/**
 * Integration tests for Veritoken contracts against a local Stellar quickstart node.
 *
 * Prerequisites:
 *   - A Stellar standalone node running on http://localhost:8000
 *     (start via `docker-compose up -d` from the repo root)
 *   - WASM binaries built under target/wasm32-unknown-unknown/release/
 *
 * Run: npm test
 */

import { describe, it, expect, beforeAll } from "vitest";
import {
  Keypair,
  Networks,
  SorobanRpc,
  TransactionBuilder,
  BASE_FEE,
  xdr,
  Operation,
  Contract,
} from "@stellar/stellar-sdk";
import * as fs from "fs";
import * as path from "path";

const RPC_URL = process.env.STELLAR_RPC_URL ?? "http://localhost:8000/soroban/rpc";
const NETWORK_PASSPHRASE = Networks.STANDALONE;

const rpc = new SorobanRpc.Server(RPC_URL, { allowHttp: true });

// Funded test account (quickstart pre-funds this key in standalone mode)
const admin = Keypair.fromSecret(
  "SCZANGBA5RLMPI7JMTP2UME5XM7JRQF6AQZH7KSGDTQR3FVTZFM7VQ"
);

// ── Helpers ───────────────────────────────────────────────────────────────────

async function uploadWasm(keypair: Keypair, wasmPath: string): Promise<string> {
  const wasmBytes = fs.readFileSync(wasmPath);
  const account = await rpc.getAccount(keypair.publicKey());
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      Operation.uploadContractWasm({ wasm: wasmBytes })
    )
    .setTimeout(30)
    .build();

  const prepared = await rpc.prepareTransaction(tx);
  prepared.sign(keypair);
  const result = await rpc.sendTransaction(prepared);
  const hash = result.hash;

  let getResult = await rpc.getTransaction(hash);
  while (getResult.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1000));
    getResult = await rpc.getTransaction(hash);
  }
  if (getResult.status !== "SUCCESS") {
    throw new Error(`Upload failed: ${JSON.stringify(getResult)}`);
  }
  const meta = getResult.resultMetaXdr;
  const parsed = xdr.TransactionMeta.fromXDR(meta, "base64");
  const wasmHash = parsed
    .v3()
    .sorobanMeta()
    ?.returnValue()
    .bytes()
    .toString("hex");
  if (!wasmHash) throw new Error("No wasm hash returned");
  return wasmHash;
}

async function deployContract(
  keypair: Keypair,
  wasmHash: string,
  constructorArgs: xdr.ScVal[]
): Promise<string> {
  const account = await rpc.getAccount(keypair.publicKey());
  const salt = Buffer.allocUnsafe(32);
  crypto.getRandomValues(salt);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      Operation.createCustomContract({
        address: new Contract(keypair.publicKey()).address(),
        wasmHash: Buffer.from(wasmHash, "hex"),
        salt,
        constructorArgs,
      })
    )
    .setTimeout(30)
    .build();

  const prepared = await rpc.prepareTransaction(tx);
  prepared.sign(keypair);
  const result = await rpc.sendTransaction(prepared);
  const hash = result.hash;

  let getResult = await rpc.getTransaction(hash);
  while (getResult.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1000));
    getResult = await rpc.getTransaction(hash);
  }
  if (getResult.status !== "SUCCESS") {
    throw new Error(`Deploy failed: ${JSON.stringify(getResult)}`);
  }
  const meta = getResult.resultMetaXdr;
  const parsed = xdr.TransactionMeta.fromXDR(meta, "base64");
  const contractId = parsed
    .v3()
    .sorobanMeta()
    ?.returnValue()
    .address()
    .contractId()
    .toString("hex");
  if (!contractId) throw new Error("No contract ID returned");
  return contractId;
}

async function invokeContract(
  keypair: Keypair,
  contractId: string,
  method: string,
  args: xdr.ScVal[]
): Promise<xdr.ScVal> {
  const account = await rpc.getAccount(keypair.publicKey());
  const contract = new Contract(contractId);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  const prepared = await rpc.prepareTransaction(tx);
  prepared.sign(keypair);
  const result = await rpc.sendTransaction(prepared);
  const hash = result.hash;

  let getResult = await rpc.getTransaction(hash);
  while (getResult.status === "NOT_FOUND") {
    await new Promise((r) => setTimeout(r, 1000));
    getResult = await rpc.getTransaction(hash);
  }
  if (getResult.status !== "SUCCESS") {
    throw new Error(`Invoke ${method} failed: ${JSON.stringify(getResult)}`);
  }
  const meta = getResult.resultMetaXdr;
  const parsed = xdr.TransactionMeta.fromXDR(meta, "base64");
  return parsed.v3().sorobanMeta()!.returnValue();
}

// ── Test suite ────────────────────────────────────────────────────────────────

const WASM_DIR = path.resolve(
  import.meta.dirname,
  "../../target/wasm32-unknown-unknown/release"
);

describe("KYC Registry lifecycle", () => {
  let kycContractId: string;

  beforeAll(async () => {
    const wasmHash = await uploadWasm(
      admin,
      path.join(WASM_DIR, "kyc_registry.wasm")
    );
    const adminAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(
          Buffer.from(admin.rawPublicKey())
        )
      )
    );
    kycContractId = await deployContract(admin, wasmHash, [adminAddr]);
  });

  it("deploys KYC registry and admin can add a verifier", async () => {
    expect(kycContractId).toBeTruthy();

    const verifier = Keypair.random();
    const verifierAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(
          Buffer.from(verifier.rawPublicKey())
        )
      )
    );

    await invokeContract(admin, kycContractId, "add_verifier", [verifierAddr]);
    const count = await invokeContract(admin, kycContractId, "verifier_count", []);
    expect(count.u32()).toBe(1);
  });
});

describe("Compliance Engine lifecycle", () => {
  let kycContractId: string;
  let ceContractId: string;

  beforeAll(async () => {
    const kycHash = await uploadWasm(
      admin,
      path.join(WASM_DIR, "kyc_registry.wasm")
    );
    const adminAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(
          Buffer.from(admin.rawPublicKey())
        )
      )
    );
    kycContractId = await deployContract(admin, kycHash, [adminAddr]);

    const ceHash = await uploadWasm(
      admin,
      path.join(WASM_DIR, "compliance_engine.wasm")
    );
    const kycAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeContract(Buffer.from(kycContractId, "hex"))
    );
    ceContractId = await deployContract(admin, ceHash, [adminAddr, kycAddr]);
  });

  it("deploys compliance engine and default rules allow transfers", async () => {
    expect(ceContractId).toBeTruthy();

    const from = Keypair.random();
    const to = Keypair.random();
    const fromAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(from.rawPublicKey()))
      )
    );
    const toAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(to.rawPublicKey()))
      )
    );
    const amount = xdr.ScVal.scvI128(
      new xdr.Int128Parts({ hi: xdr.Int64.fromString("0"), lo: xdr.Uint64.fromString("1000") })
    );

    const result = await invokeContract(admin, ceContractId, "can_transfer", [
      fromAddr,
      toAddr,
      amount,
    ]);
    expect(result.bool()).toBe(true);
  });

  it("pause blocks all transfers", async () => {
    await invokeContract(admin, ceContractId, "pause", []);

    const from = Keypair.random();
    const to = Keypair.random();
    const fromAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(from.rawPublicKey()))
      )
    );
    const toAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(to.rawPublicKey()))
      )
    );
    const amount = xdr.ScVal.scvI128(
      new xdr.Int128Parts({ hi: xdr.Int64.fromString("0"), lo: xdr.Uint64.fromString("1") })
    );

    const result = await invokeContract(admin, ceContractId, "can_transfer", [
      fromAddr,
      toAddr,
      amount,
    ]);
    expect(result.bool()).toBe(false);

    await invokeContract(admin, ceContractId, "unpause", []);
  });
});

describe("RWA Token lifecycle", () => {
  let kycContractId: string;
  let ceContractId: string;
  let rwaContractId: string;
  const investor = Keypair.random();

  beforeAll(async () => {
    const adminAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(admin.rawPublicKey()))
      )
    );

    const kycHash = await uploadWasm(admin, path.join(WASM_DIR, "kyc_registry.wasm"));
    kycContractId = await deployContract(admin, kycHash, [adminAddr]);

    const verifier = Keypair.random();
    const verifierAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(verifier.rawPublicKey()))
      )
    );
    await invokeContract(admin, kycContractId, "add_verifier", [verifierAddr]);

    const ceHash = await uploadWasm(admin, path.join(WASM_DIR, "compliance_engine.wasm"));
    const kycAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeContract(Buffer.from(kycContractId, "hex"))
    );
    ceContractId = await deployContract(admin, ceHash, [adminAddr, kycAddr]);

    const rwaHash = await uploadWasm(admin, path.join(WASM_DIR, "rwa_token.wasm"));
    const ceAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeContract(Buffer.from(ceContractId, "hex"))
    );
    rwaContractId = await deployContract(admin, rwaHash, [
      adminAddr,
      xdr.ScVal.scvU32(7),
      xdr.ScVal.scvString("Veritoken RWA"),
      xdr.ScVal.scvString("VTRWA"),
      xdr.ScVal.scvString("property"),
      xdr.ScVal.scvAddress(
        xdr.ScAddress.scAddressTypeContract(Buffer.from(kycContractId, "hex"))
      ),
      ceAddr,
      xdr.ScVal.scvVoid(),
    ]);
  });

  it("deploys RWA token with correct metadata", async () => {
    expect(rwaContractId).toBeTruthy();
    const name = await invokeContract(admin, rwaContractId, "name", []);
    expect(name.str().toString()).toBe("Veritoken RWA");
  });
});

describe("Invoice Token multi-invoice lifecycle", () => {
  let kycContractId: string;
  let ceContractId: string;
  let invoiceContractId: string;

  beforeAll(async () => {
    const adminAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeAccount(
        xdr.PublicKey.publicKeyTypeEd25519(Buffer.from(admin.rawPublicKey()))
      )
    );

    const kycHash = await uploadWasm(admin, path.join(WASM_DIR, "kyc_registry.wasm"));
    kycContractId = await deployContract(admin, kycHash, [adminAddr]);

    const ceHash = await uploadWasm(admin, path.join(WASM_DIR, "compliance_engine.wasm"));
    const kycAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeContract(Buffer.from(kycContractId, "hex"))
    );
    ceContractId = await deployContract(admin, ceHash, [adminAddr, kycAddr]);

    const invoiceHash = await uploadWasm(
      admin,
      path.join(WASM_DIR, "invoice_token.wasm")
    );
    const ceAddr = xdr.ScVal.scvAddress(
      xdr.ScAddress.scAddressTypeContract(Buffer.from(ceContractId, "hex"))
    );

    const initialMeta = xdr.ScVal.scvMap([
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("invoice_id"),
        val: xdr.ScVal.scvString("INV-001"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("issuer"),
        val: xdr.ScVal.scvString("Acme Corp"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("debtor"),
        val: xdr.ScVal.scvString("Globex"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("face_value_usd"),
        val: xdr.ScVal.scvI128(
          new xdr.Int128Parts({
            hi: xdr.Int64.fromString("0"),
            lo: xdr.Uint64.fromString("1000000000000"),
          })
        ),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("discount_rate_bps"),
        val: xdr.ScVal.scvU32(250),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("due_date"),
        val: xdr.ScVal.scvU64(xdr.Uint64.fromString("1900000000")),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("currency"),
        val: xdr.ScVal.scvString("USD"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("ipfs_doc_hash"),
        val: xdr.ScVal.scvString("Qm..."),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("transfer_fee_bps"),
        val: xdr.ScVal.scvU32(0),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("fee_recipient"),
        val: xdr.ScVal.scvVoid(),
      }),
    ]);

    invoiceContractId = await deployContract(admin, invoiceHash, [
      adminAddr,
      xdr.ScVal.scvAddress(
        xdr.ScAddress.scAddressTypeContract(Buffer.from(kycContractId, "hex"))
      ),
      ceAddr,
      initialMeta,
    ]);
  });

  it("deploys invoice token and lists initial invoice", async () => {
    expect(invoiceContractId).toBeTruthy();

    const invoices = await invokeContract(
      admin,
      invoiceContractId,
      "list_invoices",
      [xdr.ScVal.scvU32(0), xdr.ScVal.scvU32(10)]
    );
    expect(invoices.vec()!.length).toBe(1);
  });

  it("creates a second invoice", async () => {
    const secondMeta = xdr.ScVal.scvMap([
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("invoice_id"),
        val: xdr.ScVal.scvString("INV-002"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("issuer"),
        val: xdr.ScVal.scvString("Beta Corp"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("debtor"),
        val: xdr.ScVal.scvString("Delta Ltd"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("face_value_usd"),
        val: xdr.ScVal.scvI128(
          new xdr.Int128Parts({
            hi: xdr.Int64.fromString("0"),
            lo: xdr.Uint64.fromString("500000000000"),
          })
        ),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("discount_rate_bps"),
        val: xdr.ScVal.scvU32(100),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("due_date"),
        val: xdr.ScVal.scvU64(xdr.Uint64.fromString("1900000000")),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("currency"),
        val: xdr.ScVal.scvString("USD"),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("ipfs_doc_hash"),
        val: xdr.ScVal.scvString(""),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("transfer_fee_bps"),
        val: xdr.ScVal.scvU32(0),
      }),
      new xdr.ScMapEntry({
        key: xdr.ScVal.scvSymbol("fee_recipient"),
        val: xdr.ScVal.scvVoid(),
      }),
    ]);

    await invokeContract(admin, invoiceContractId, "create_invoice", [secondMeta]);

    const invoices = await invokeContract(
      admin,
      invoiceContractId,
      "list_invoices",
      [xdr.ScVal.scvU32(0), xdr.ScVal.scvU32(10)]
    );
    expect(invoices.vec()!.length).toBe(2);
  });
});
