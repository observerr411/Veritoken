/**
 * Shared helpers used by every contract client.
 *
 * Soroban read calls: build a no-auth transaction → simulateTransaction → decode retval.
 * Soroban write calls: build the transaction → simulateAndSend → return.
 *
 * The dummy source account used for simulation is the well-known fee-bump source from
 * the Stellar docs. It carries a sequence number of "0" and never needs to be funded
 * because simulation does not submit to the network.
 */

import {
  Contract,
  TransactionBuilder,
  Account,
  xdr,
  scValToNative,
  nativeToScVal,
  type rpc,
} from "@stellar/stellar-sdk";
import { NETWORK_PASSPHRASE, simulateAndSend } from "../stellar";

// A stable dummy address used only for read simulations.
const DUMMY_SOURCE = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN";

export type SignTx = (xdr: string) => Promise<string>;

// ── helpers ──────────────────────────────────────────────────────────────────

/** Build a single-operation transaction ready for simulation or submission. */
export function buildTx(
  contractId: string,
  method: string,
  args: xdr.ScVal[],
  source: string,
  sequence: string
): string {
  const account = new Account(source, sequence);
  const contract = new Contract(contractId);
  const tx = new TransactionBuilder(account, {
    fee: "100",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();
  return tx.toXDR();
}

/** Simulate a read-only call and return the decoded native JS value. */
export async function readCall<T>(
  server: rpc.Server,
  contractId: string,
  method: string,
  args: xdr.ScVal[]
): Promise<T> {
  const xdrTx = buildTx(contractId, method, args, DUMMY_SOURCE, "0");
  const sim = await server.simulateTransaction(
    TransactionBuilder.fromXDR(xdrTx, NETWORK_PASSPHRASE)
  );

  if ("error" in sim && sim.error) {
    throw new Error(`Simulation error calling ${method}: ${sim.error}`);
  }

  const result = (sim as { result?: { retval: xdr.ScVal } }).result;
  if (!result?.retval) {
    throw new Error(`No return value from ${method}`);
  }
  return scValToNative(result.retval) as T;
}

/** Build, simulate, and submit a state-mutating call. Returns once confirmed. */
export async function writeCall(
  _server: rpc.Server,
  contractId: string,
  method: string,
  args: xdr.ScVal[],
  senderAddress: string,
  senderSequence: string,
  signTx: SignTx
): Promise<void> {
  const xdrTx = buildTx(contractId, method, args, senderAddress, senderSequence);
  await simulateAndSend(xdrTx, signTx);
}

// ── ScVal converters ─────────────────────────────────────────────────────────

export const toAddress = (addr: string): xdr.ScVal =>
  nativeToScVal(addr, { type: "address" });

export const toU32 = (n: number): xdr.ScVal =>
  nativeToScVal(n, { type: "u32" });

export const toU64 = (n: bigint | number): xdr.ScVal =>
  nativeToScVal(BigInt(n), { type: "u64" });

export const toI128 = (n: bigint | number): xdr.ScVal =>
  nativeToScVal(BigInt(n), { type: "i128" });

export const toString = (s: string): xdr.ScVal =>
  nativeToScVal(s, { type: "string" });

export const toBool = (b: boolean): xdr.ScVal =>
  nativeToScVal(b, { type: "bool" });

/** Fetch the current sequence number for a Stellar account. */
export async function fetchSequence(server: rpc.Server, address: string): Promise<string> {
  const account = await server.getAccount(address);
  return (account as unknown as { sequence: string }).sequence;
}
