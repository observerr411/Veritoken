/**
 * Typed client for the compliance-engine Soroban contract.
 *
 * Contract functions covered:
 *   Read:  get_rules, is_blocklisted, can_transfer, holder_count
 *   Write: set_rules, add_to_blocklist, remove_from_blocklist, pause, unpause,
 *          register_holder, unregister_holder
 */

import type { rpc } from "@stellar/stellar-sdk";
import type { ComplianceRules } from "../../types";
import {
  readCall,
  writeCall,
  fetchSequence,
  toAddress,
  toI128,
  type SignTx,
} from "./base";
import { nativeToScVal, xdr } from "@stellar/stellar-sdk";

// ── Local struct encoder ──────────────────────────────────────────────────────

/**
 * Encode a `ComplianceRules` struct as a Soroban map ScVal.
 * Field order must match the #[contracttype] declaration in the contract.
 */
function encodeRules(rules: ComplianceRules): xdr.ScVal {
  return nativeToScVal(
    {
      max_transfer_amount: rules.max_transfer_amount,
      min_holding_period: Number(rules.min_holding_period),
      max_holders: rules.max_holders,
      require_same_jurisdiction: rules.require_same_jurisdiction,
      paused: rules.paused,
    },
    {
      type: {
        max_transfer_amount: ["i128"],
        min_holding_period: ["u64"],
        max_holders: ["u32"],
        require_same_jurisdiction: ["bool"],
        paused: ["bool"],
      },
    }
  );
}

export class ComplianceEngineClient {
  constructor(
    private readonly contractId: string,
    private readonly server: rpc.Server
  ) {}

  // ── Read methods ──────────────────────────────────────────────────────────

  /** Returns the currently active compliance rule set. */
  async getRules(): Promise<ComplianceRules> {
    return readCall<ComplianceRules>(
      this.server,
      this.contractId,
      "get_rules",
      []
    );
  }

  /** Returns true when `addr` is on the transfer blocklist. */
  async isBlocklisted(addr: string): Promise<boolean> {
    return readCall<boolean>(this.server, this.contractId, "is_blocklisted", [
      toAddress(addr),
    ]);
  }

  /**
   * Returns true when the transfer would pass all compliance checks.
   * Does NOT submit a transaction.
   */
  async canTransfer(from: string, to: string, amount: bigint): Promise<boolean> {
    return readCall<boolean>(this.server, this.contractId, "can_transfer", [
      toAddress(from),
      toAddress(to),
      toI128(amount),
    ]);
  }

  /** Returns the current count of registered holders. */
  async holderCount(): Promise<number> {
    return readCall<number>(this.server, this.contractId, "holder_count", []);
  }

  // ── Write methods ─────────────────────────────────────────────────────────

  /** Replace the active compliance rules. Admin-only on-chain. */
  async setRules(
    adminAddress: string,
    rules: ComplianceRules,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "set_rules",
      [encodeRules(rules)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Add `addr` to the transfer blocklist. Admin-only on-chain. */
  async addToBlocklist(
    adminAddress: string,
    addr: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "add_to_blocklist",
      [toAddress(addr)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Remove `addr` from the transfer blocklist. Admin-only on-chain. */
  async removeFromBlocklist(
    adminAddress: string,
    addr: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "remove_from_blocklist",
      [toAddress(addr)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Halt all transfers. Admin-only on-chain. */
  async pause(adminAddress: string, signTx: SignTx): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "pause",
      [],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Resume transfers. Admin-only on-chain. */
  async unpause(adminAddress: string, signTx: SignTx): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "unpause",
      [],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Register a new holder (called after mint/transfer-in). */
  async registerHolder(
    callerAddress: string,
    addr: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, callerAddress);
    return writeCall(
      this.server,
      this.contractId,
      "register_holder",
      [toAddress(addr)],
      callerAddress,
      seq,
      signTx
    );
  }

  /** Unregister a holder whose balance has dropped to zero. */
  async unregisterHolder(
    callerAddress: string,
    addr: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, callerAddress);
    return writeCall(
      this.server,
      this.contractId,
      "unregister_holder",
      [toAddress(addr)],
      callerAddress,
      seq,
      signTx
    );
  }
}


