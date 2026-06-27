/**
 * Typed client for the property-token Soroban contract.
 *
 * Contract functions covered:
 *   Read:  get_meta, name, symbol, decimals, balance, total_shares, pending_dividend
 *   Write: mint, transfer, deposit_dividend, claim_dividend
 */

import type { rpc } from "@stellar/stellar-sdk";
import type { PropertyMeta } from "../../types";
import {
  readCall,
  writeCall,
  fetchSequence,
  toAddress,
  toI128,
  type SignTx,
} from "./base";

export class PropertyTokenClient {
  constructor(
    private readonly contractId: string,
    private readonly server: rpc.Server
  ) {}

  // ── Read methods ──────────────────────────────────────────────────────────

  /** Returns the property metadata stored on-chain at deploy time. */
  async getMeta(): Promise<PropertyMeta> {
    return readCall<PropertyMeta>(
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

  /** Returns the decimal precision (0 for whole shares). */
  async decimals(): Promise<number> {
    return readCall<number>(this.server, this.contractId, "decimals", []);
  }

  /** Returns the share balance of `addr`. */
  async balance(addr: string): Promise<bigint> {
    return readCall<bigint>(this.server, this.contractId, "balance", [
      toAddress(addr),
    ]);
  }

  /** Returns the total number of shares in circulation. */
  async totalShares(): Promise<bigint> {
    return readCall<bigint>(
      this.server,
      this.contractId,
      "total_shares",
      []
    );
  }

  /**
   * Returns the pending (unclaimed) dividend balance for `holder` in stroops.
   * This is a read-only call and does not submit a transaction.
   */
  async pendingDividend(holder: string): Promise<bigint> {
    return readCall<bigint>(
      this.server,
      this.contractId,
      "pending_dividend",
      [toAddress(holder)]
    );
  }

  // ── Write methods ─────────────────────────────────────────────────────────

  /** Mint `shares` to `to`. Admin-only on-chain; subject to KYC and tier checks. */
  async mint(
    adminAddress: string,
    to: string,
    shares: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "mint",
      [toAddress(to), toI128(shares)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Transfer `shares` from `from` to `to`. Requires `from`'s auth. */
  async transfer(
    fromAddress: string,
    to: string,
    shares: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, fromAddress);
    return writeCall(
      this.server,
      this.contractId,
      "transfer",
      [toAddress(fromAddress), toAddress(to), toI128(shares)],
      fromAddress,
      seq,
      signTx
    );
  }

  /**
   * Deposit a dividend amount (in stroops) to be distributed pro-rata.
   * Admin-only on-chain.
   */
  async depositDividend(
    adminAddress: string,
    amount: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "deposit_dividend",
      [toI128(amount)],
      adminAddress,
      seq,
      signTx
    );
  }

  /**
   * Claim all accrued dividends for `holder`. Returns the claimed amount.
   * Requires `holder`'s auth.
   */
  async claimDividend(
    holderAddress: string,
    signTx: SignTx
  ): Promise<bigint> {
    // Snapshot the pending amount before the write so we can return it.
    // claim_dividend zeroes the accumulator on-chain; the return value equals
    // whatever was pending at the time the transaction lands.
    const pending = await readCall<bigint>(
      this.server,
      this.contractId,
      "pending_dividend",
      [toAddress(holderAddress)]
    );
    const seq = await fetchSequence(this.server, holderAddress);
    await writeCall(
      this.server,
      this.contractId,
      "claim_dividend",
      [toAddress(holderAddress)],
      holderAddress,
      seq,
      signTx
    );
    return pending;
  }
}
