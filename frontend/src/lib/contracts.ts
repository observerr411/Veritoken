/**
 * Typed contract clients for Veritoken's Soroban contracts.
 *
 * Each client method builds an XDR transaction, simulates it, and — for
 * state-mutating calls — signs and sends it via the provided `signTx`
 * callback (Freighter's signTransaction wrapper).
 *
 * Read-only methods (prefixed `get`/`query`) skip signing and parse the
 * simulation result directly, so they work without a connected wallet.
 */

import {
  Contract,
  TransactionBuilder,
  BASE_FEE,
  xdr,
  scValToNative,
  nativeToScVal,
  Address,
} from "@stellar/stellar-sdk";
import type { rpc as StellarRpc } from "@stellar/stellar-sdk";
import {
  server,
  NETWORK_PASSPHRASE,
  CONTRACT_IDS,
  simulateAndSend,
} from "./stellar";
import type { ProjectMeta, RetirementReceipt } from "../types";

// ── helpers ──────────────────────────────────────────────────────────────────

/** Build a transaction that calls a single contract function. */
async function buildTx(
  contractId: string,
  method: string,
  args: xdr.ScVal[],
  sourceAddress: string
): Promise<string> {
  const account = await server.getAccount(sourceAddress);
  const contract = new Contract(contractId);
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(300)
    .build();
  return tx.toXDR();
}

/**
 * Simulate a read-only call and return the raw ScVal result.
 * Throws if the simulation reports an error.
 */
async function simulateRead(
  contractId: string,
  method: string,
  args: xdr.ScVal[],
  sourceAddress: string
): Promise<xdr.ScVal> {
  const xdrTx = await buildTx(contractId, method, args, sourceAddress);
  const sim = await server.simulateTransaction(
    TransactionBuilder.fromXDR(xdrTx, NETWORK_PASSPHRASE)
  ) as StellarRpc.Api.SimulateTransactionResponse;

  // Type-narrow to success
  if ("error" in sim) {
    throw new Error(`Simulation error: ${(sim as { error: string }).error}`);
  }
  const successSim = sim as StellarRpc.Api.SimulateTransactionSuccessResponse;
  if (!successSim.result) {
    throw new Error("Simulation returned no result");
  }
  return successSim.result.retval;
}

// ── CarbonTokenClient ────────────────────────────────────────────────────────

export class CarbonTokenClient {
  private id: string;

  constructor(contractId: string) {
    this.id = contractId;
  }

  // ── Typed read helpers ────────────────────────────────────────────────────

  async getMeta(sourceAddress: string): Promise<ProjectMeta> {
    const val = await simulateRead(this.id, "get_meta", [], sourceAddress);
    return scValToNative(val) as ProjectMeta;
  }

  async balance(address: string, sourceAddress: string): Promise<bigint> {
    const val = await simulateRead(
      this.id,
      "balance",
      [new Address(address).toScVal()],
      sourceAddress
    );
    return scValToNative(val) as bigint;
  }

  async totalSupply(sourceAddress: string): Promise<bigint> {
    const val = await simulateRead(this.id, "total_supply", [], sourceAddress);
    return scValToNative(val) as bigint;
  }

  async totalRetired(sourceAddress: string): Promise<bigint> {
    const val = await simulateRead(this.id, "total_retired", [], sourceAddress);
    return scValToNative(val) as bigint;
  }

  async retirementCount(sourceAddress: string): Promise<number> {
    const val = await simulateRead(this.id, "retirement_count", [], sourceAddress);
    return Number(scValToNative(val));
  }

  async getReceipt(index: number, sourceAddress: string): Promise<RetirementReceipt> {
    const val = await simulateRead(
      this.id,
      "get_receipt",
      [nativeToScVal(index, { type: "u32" })],
      sourceAddress
    );
    return scValToNative(val) as RetirementReceipt;
  }

  async getReceipts(
    start: number,
    limit: number,
    sourceAddress: string
  ): Promise<RetirementReceipt[]> {
    const val = await simulateRead(
      this.id,
      "get_receipts",
      [
        nativeToScVal(start, { type: "u32" }),
        nativeToScVal(limit, { type: "u32" }),
      ],
      sourceAddress
    );
    return scValToNative(val) as RetirementReceipt[];
  }

  // ── State-mutating calls ──────────────────────────────────────────────────

  async mint(
    to: string,
    amount: bigint,
    signTx: (xdr: string) => Promise<string>
  ): Promise<void> {
    const xdrTx = await buildTx(
      this.id,
      "mint",
      [new Address(to).toScVal(), nativeToScVal(amount, { type: "i128" })],
      to
    );
    await simulateAndSend(xdrTx, signTx);
  }

  async transfer(
    from: string,
    to: string,
    amount: bigint,
    signTx: (xdr: string) => Promise<string>
  ): Promise<void> {
    const xdrTx = await buildTx(
      this.id,
      "transfer",
      [
        new Address(from).toScVal(),
        new Address(to).toScVal(),
        nativeToScVal(amount, { type: "i128" }),
      ],
      from
    );
    await simulateAndSend(xdrTx, signTx);
  }

  async retire(
    retiree: string,
    amount: bigint,
    beneficiary: string,
    reason: string,
    signTx: (xdr: string) => Promise<string>
  ): Promise<RetirementReceipt> {
    const xdrTx = await buildTx(
      this.id,
      "retire",
      [
        new Address(retiree).toScVal(),
        nativeToScVal(amount, { type: "i128" }),
        nativeToScVal(beneficiary, { type: "string" }),
        nativeToScVal(reason, { type: "string" }),
      ],
      retiree
    );
    const txResult = await simulateAndSend(xdrTx, signTx);
    // Extract return value from the transaction result
    const resultMeta = txResult.resultMetaXdr;
    const ops = resultMeta.v3()?.sorobanMeta()?.returnValue();
    if (!ops) {
      throw new Error("No return value in retirement transaction");
    }
    return scValToNative(ops) as RetirementReceipt;
  }
}

// ── Singleton factory ────────────────────────────────────────────────────────

/** Pre-built clients bound to the contract IDs from .env */
export const contracts = {
  get carbonToken() {
    return new CarbonTokenClient(CONTRACT_IDS.carbonToken);
  },
};
