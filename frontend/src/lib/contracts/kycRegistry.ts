/**
 * Typed client for the kyc-registry Soroban contract.
 *
 * Contract functions covered:
 *   Read:  is_approved, get_record, get_tier, verifier_count, get_verifiers, verifier_list_pub
 *   Write: approve, reject, revoke, update_tier, add_verifier, remove_verifier
 */

import type { rpc } from "@stellar/stellar-sdk";
import type { KycRecord } from "../../types";
import {
  readCall,
  writeCall,
  fetchSequence,
  toAddress,
  toU32,
  toU64,
  toString,
  type SignTx,
} from "./base";

export class KycRegistryClient {
  constructor(
    private readonly contractId: string,
    private readonly server: rpc.Server
  ) {}

  // ── Read methods ──────────────────────────────────────────────────────────

  /** Returns true when `addr` has an active, non-expired KYC approval. */
  async isApproved(addr: string): Promise<boolean> {
    return readCall<boolean>(this.server, this.contractId, "is_approved", [
      toAddress(addr),
    ]);
  }

  /** Returns the full KYC record for `addr`. Throws if no record exists. */
  async getRecord(addr: string): Promise<KycRecord> {
    return readCall<KycRecord>(this.server, this.contractId, "get_record", [
      toAddress(addr),
    ]);
  }

  /** Returns the KYC tier of `addr`. */
  async getTier(addr: string): Promise<number> {
    return readCall<number>(this.server, this.contractId, "get_tier", [
      toAddress(addr),
    ]);
  }

  /** Returns the total number of registered verifiers. */
  async verifierCount(): Promise<number> {
    return readCall<number>(this.server, this.contractId, "verifier_count", []);
  }

  /**
   * Returns a page of verifier addresses.
   * @param start Zero-based offset into the verifier list.
   * @param limit Maximum entries to return; capped at 20 on-chain.
   */
  async getVerifiers(start: number, limit: number): Promise<string[]> {
    return readCall<string[]>(this.server, this.contractId, "get_verifiers", [
      toU32(start),
      toU32(limit),
    ]);
  }

  /** Returns the complete verifier list. Prefer `getVerifiers` for large registries. */
  async verifierListPub(): Promise<string[]> {
    return readCall<string[]>(
      this.server,
      this.contractId,
      "verifier_list_pub",
      []
    );
  }

  // ── Write methods ─────────────────────────────────────────────────────────

  /**
   * Approve KYC for `subject` as the given `verifier`.
   * @param expiry Unix timestamp after which the approval expires; 0 = no expiry.
   */
  async approve(
    verifier: string,
    subject: string,
    tier: number,
    expiry: bigint,
    jurisdiction: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, verifier);
    return writeCall(
      this.server,
      this.contractId,
      "approve",
      [
        toAddress(verifier),
        toAddress(subject),
        toU32(tier),
        toU64(expiry),
        toString(jurisdiction),
      ],
      verifier,
      seq,
      signTx
    );
  }

  /** Mark `subject`'s KYC record as Rejected. */
  async reject(verifier: string, subject: string, signTx: SignTx): Promise<void> {
    const seq = await fetchSequence(this.server, verifier);
    return writeCall(
      this.server,
      this.contractId,
      "reject",
      [toAddress(verifier), toAddress(subject)],
      verifier,
      seq,
      signTx
    );
  }

  /** Mark `subject`'s KYC record as Revoked. */
  async revoke(verifier: string, subject: string, signTx: SignTx): Promise<void> {
    const seq = await fetchSequence(this.server, verifier);
    return writeCall(
      this.server,
      this.contractId,
      "revoke",
      [toAddress(verifier), toAddress(subject)],
      verifier,
      seq,
      signTx
    );
  }

  /**
   * Update only the tier of an already-Approved KYC record.
   * Panics on-chain if the subject is not currently Approved.
   */
  async updateTier(
    verifier: string,
    subject: string,
    newTier: number,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, verifier);
    return writeCall(
      this.server,
      this.contractId,
      "update_tier",
      [toAddress(verifier), toAddress(subject), toU32(newTier)],
      verifier,
      seq,
      signTx
    );
  }

  /** Register `verifier` as an authorized KYC verifier. Admin-only on-chain. */
  async addVerifier(
    adminAddress: string,
    verifier: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "add_verifier",
      [toAddress(verifier)],
      adminAddress,
      seq,
      signTx
    );
  }

  /** Remove `verifier` from the authorized verifier list. Admin-only on-chain. */
  async removeVerifier(
    adminAddress: string,
    verifier: string,
    signTx: SignTx
  ): Promise<void> {
    const seq = await fetchSequence(this.server, adminAddress);
    return writeCall(
      this.server,
      this.contractId,
      "remove_verifier",
      [toAddress(verifier)],
      adminAddress,
      seq,
      signTx
    );
  }
}
