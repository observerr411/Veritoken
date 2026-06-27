import { Address, nativeToScVal, rpc } from "@stellar/stellar-sdk";
import { BaseContractClient, scVal } from "./base.js";
import type { KycRecord } from "../types.js";

export class KycRegistryClient extends BaseContractClient {
  constructor(contractId: string, server: rpc.Server, networkPassphrase: string) {
    super(contractId, server, networkPassphrase);
  }

  // ── Read API ─────────────────────────────────────────────────────────────

  async isApproved(addr: string): Promise<boolean> {
    return scVal<boolean>(
      await this.simulate("is_approved", [new Address(addr).toScVal()]),
    );
  }

  async getRecord(addr: string): Promise<KycRecord> {
    return scVal<KycRecord>(
      await this.simulate("get_record", [new Address(addr).toScVal()]),
    );
  }

  async getTier(addr: string): Promise<number> {
    return scVal<number>(
      await this.simulate("get_tier", [new Address(addr).toScVal()]),
    );
  }

  // ── Transaction builders (return operation XDR for signing) ───────────────

  buildApproveXdr(
    verifier: string,
    subject: string,
    tier: number,
    expiry: bigint,
    jurisdiction: string,
  ): string {
    return this.buildCallXdr("approve", [
      new Address(verifier).toScVal(),
      new Address(subject).toScVal(),
      nativeToScVal(tier, { type: "u32" }),
      nativeToScVal(expiry, { type: "u64" }),
      nativeToScVal(jurisdiction, { type: "string" }),
    ]);
  }

  buildAddVerifierXdr(verifier: string): string {
    return this.buildCallXdr("add_verifier", [new Address(verifier).toScVal()]);
  }

  buildRemoveVerifierXdr(verifier: string): string {
    return this.buildCallXdr("remove_verifier", [new Address(verifier).toScVal()]);
  }

  buildRevokeXdr(verifier: string, subject: string): string {
    return this.buildCallXdr("revoke", [
      new Address(verifier).toScVal(),
      new Address(subject).toScVal(),
    ]);
  }

  buildRejectXdr(verifier: string, subject: string): string {
    return this.buildCallXdr("reject", [
      new Address(verifier).toScVal(),
      new Address(subject).toScVal(),
    ]);
  }
}
