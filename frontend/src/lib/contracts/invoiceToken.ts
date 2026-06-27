/**
 * Typed client for the invoice-token Soroban contract.
 *
 * Contract functions covered:
 *   Read:  get_meta, name, symbol, decimals, balance, total_supply, is_settled, allowance
 *   Write: issue, settle, redeem, transfer, transfer_from, approve
 */

import type { rpc } from "@stellar/stellar-sdk";
import type { InvoiceMeta } from "../../types";
import {
  readCall,
  writeCall,
  fetchSequence,
  toAddress,
  toI128,
  toU32,
  type SignTx,
} from "./base";

// ── Local struct encoder ──────────────────────────────────────────────────────
// (Retained for future use — invoice metadata is set at deploy time via the
//  __constructor and cannot be updated post-deploy.)

export class InvoiceTokenClient {
  constructor(
    private readonly contractId: string,
    private readonly server: rpc.Server
  ) {}

  // ── Read methods ──────────────────────────────────────────────────────────

  /** Returns the invoice metadata stored on-chain at deploy time. */
  async getMeta(): Promise<InvoiceMeta> {
    return readCall<InvoiceMeta>(this.server, this.contractId, "get_meta", []);
  }

  /** Returns the token name. */
  async name(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "name", []);
  }

  /** Returns the token symbol. */
  async symbol(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "symbol", []);
  }

  /** Returns the decimal precision (7 for invoice tokens). */
  async decimals(): Promise<number> {
    return readCall<number>(this.server, this.contractId, "decimals", []);
  }

  /** Returns the token balance of `addr`. */
  async balance(addr: string): Promise<bigint> {
    return readCall<bigint>(this.server, this.contractId, "balance", [
      toAddress(addr),
    ]);
  }

  /** Returns the current total supply. */
  async totalSupply(): Promise<bigint> {
    return readCall<bigint>(
      this.server,
      this.contractId,
      "total_supply",
      []
    );
  }

  /** Returns true when the invoice has been settled and redemption is open. */
  async isSettled(): Promise<boolean> {
    return readCall<boolean>(
      this.server,
      this.contractId,
      "is_settled",
      []
    );
  }

  /** Returns the current allowance granted by `from` to `spender`. */
  async allowance(from: string, spender: string): Promise<bigint> {
    return readCall<bigint>(this.server, this.contractId, "allowance", [
      toAddress(from),
      toAddress(spender),
    ]);
  }

  // ── Write methods ─────────────────────────────────────────────────────────

  /** Mint invoice tokens to `to`. Admin-only on-chain. */
  async issue(
    adminAddress: string,
    to: string,
    amount: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "issue",
      [toAddress(to), toI128(amount)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Mark the invoice as settled and open redemption. Admin-only on-chain. */
  async settle(adminAddress: string, signTx: SignTx): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "settle",
      [],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Burn `amount` tokens upon redemption. Requires invoice to be settled. */
  async redeem(
    fromAddress: string,
    amount: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, fromAddress);
    return writeCall(
      this.server,
      this.contractId,
      "redeem",
      [toAddress(fromAddress), toI128(amount)],
      fromAddress,
      seq,
      signTx
    );
  }

  /** Transfer `amount` tokens from `from` to `to`. Requires `from`'s auth. */
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

  /** Transfer using a pre-approved allowance. Requires `spender`'s auth. */
  async transferFrom(
    spenderAddress: string,
    from: string,
    to: string,
    amount: bigint,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, spenderAddress);
    return writeCall(
      this.server,
      this.contractId,
      "transfer_from",
      [toAddress(spenderAddress), toAddress(from), toAddress(to), toI128(amount)],
      spenderAddress,
      seq,
      signTx
    );
  }

  /**
   * Grant `spender` an allowance of `amount` expiring at `expirationLedger`.
   * Requires `fromAddress`'s auth.
   */
  async approve(
    fromAddress: string,
    spender: string,
    amount: bigint,
    expirationLedger: number,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, fromAddress);
    return writeCall(
      this.server,
      this.contractId,
      "approve",
      [toAddress(fromAddress), toAddress(spender), toI128(amount), toU32(expirationLedger)],
      fromAddress,
      seq,
      signTx
    );
  }
}

