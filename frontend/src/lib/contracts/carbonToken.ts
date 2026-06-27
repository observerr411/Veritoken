/**
 * Typed client for the carbon-credit-token Soroban contract.
 *
 * Contract functions covered:
 *   Read:  get_meta, name, symbol, decimals, balance, total_supply, total_retired,
 *          retirement_count, get_receipt, get_receipts
 *   Write: mint, transfer, retire
 */

import type { rpc } from "@stellar/stellar-sdk";
import type { ProjectMeta, RetirementReceipt } from "../../types";
import {
  readCall,
  writeCall,
  fetchSequence,
  toAddress,
  toI128,
  toU32,
  toString,
  type SignTx,
} from "./base";

export class CarbonTokenClient {
  constructor(
    private readonly contractId: string,
    private readonly server: rpc.Server
  ) {}

  // ── Read methods ──────────────────────────────────────────────────────────

  /** Returns the carbon project metadata stored on-chain at deploy time. */
  async getMeta(): Promise<ProjectMeta> {
    return readCall<ProjectMeta>(
      this.server,
      this.contractId,
      "get_meta",
      []
    );
  }

  /** Returns the token name. */
  async name(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "name", []);
  }

  /** Returns the token symbol. */
  async symbol(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "symbol", []);
  }

  /** Returns the decimal precision (0 — whole tonnes). */
  async decimals(): Promise<number> {
    return readCall<number>(this.server, this.contractId, "decimals", []);
  }

  /** Returns the carbon credit balance of `addr`. */
  async balance(addr: string): Promise<bigint> {
    return readCall<bigint>(this.server, this.contractId, "balance", [
      toAddress(addr),
    ]);
  }

  /** Returns the current total supply of live (un-retired) credits. */
  async totalSupply(): Promise<bigint> {
    return readCall<bigint>(
      this.server,
      this.contractId,
      "total_supply",
      []
    );
  }

  /** Returns the cumulative amount of retired credits. */
  async totalRetired(): Promise<bigint> {
    return readCall<bigint>(
      this.server,
      this.contractId,
      "total_retired",
      []
    );
  }

  /** Returns the total number of retirement receipts stored on-chain. */
  async retirementCount(): Promise<number> {
    return readCall<number>(
      this.server,
      this.contractId,
      "retirement_count",
      []
    );
  }

  /** Returns a single retirement receipt by its zero-based index. */
  async getReceipt(index: number): Promise<RetirementReceipt> {
    return readCall<RetirementReceipt>(
      this.server,
      this.contractId,
      "get_receipt",
      [toU32(index)]
    );
  }

  /**
   * Returns up to `limit` retirement receipts starting at `start`.
   * Limit is capped at 100 on-chain.
   */
  async getReceipts(start: number, limit: number): Promise<RetirementReceipt[]> {
    return readCall<RetirementReceipt[]>(
      this.server,
      this.contractId,
      "get_receipts",
      [toU32(start), toU32(limit)]
    );
  }

  // ── Write methods ─────────────────────────────────────────────────────────

  /** Mint `amount` carbon credits to `to`. Admin-only on-chain. */
  async mint(
    adminAddress: string,
    to: string,
    amount: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "mint",
      [toAddress(to), toI128(amount)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Transfer `amount` credits from `from` to `to`. Requires `from`'s auth. */
  async transfer(
    fromAddress: string,
    to: string,
    amount: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, fromAddress);
    return writeCall(
      this.server,
      this.contractId,
      "transfer",
      [toAddress(fromAddress), toAddress(to), toI128(amount)],
      fromAddress,
      seq,
      signTx
    );
  }

  /**
   * Permanently retire `amount` credits on behalf of `retiree`.
   * Creates an on-chain retirement receipt and emits a `retired` event.
   * Requires `retiree`'s auth.
   */
  async retire(
    retireeAddress: string,
    amount: bigint,
    beneficiary: string,
    reason: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, retireeAddress);
    return writeCall(
      this.server,
      this.contractId,
      "retire",
      [
        toAddress(retireeAddress),
        toI128(amount),
        toString(beneficiary),
        toString(reason),
      ],
      retireeAddress,
      seq,
      signTx
    );
  }
}
