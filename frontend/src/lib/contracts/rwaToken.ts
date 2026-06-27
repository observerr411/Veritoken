/**
 * Typed client for the rwa-token Soroban contract.
 *
 * Contract functions covered:
 *   Read: asset_type, kyc_registry, compliance_engine, get_compliance_metadata
 */

import type { rpc } from "@stellar/stellar-sdk";
import { nativeToScVal } from "@stellar/stellar-sdk";
import { readCall } from "./base";

export class RwaTokenClient {
  constructor(
    private readonly contractId: string,
    private readonly server: rpc.Server
  ) {}

  /** Returns the asset type string (e.g. "invoice", "property", "carbon_credit"). */
  async assetType(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "asset_type", []);
  }

  /** Returns the KYC registry contract address. */
  async kycRegistry(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "kyc_registry", []);
  }

  /** Returns the compliance engine contract address. */
  async complianceEngine(): Promise<string> {
    return readCall<string>(this.server, this.contractId, "compliance_engine", []);
  }

  /**
   * Returns a compliance metadata value by key (e.g. "legal_entity",
   * "governing_law", "isin"). Returns an empty string when unset.
   */
  async getComplianceMetadata(key: string): Promise<string> {
    return readCall<string>(this.server, this.contractId, "get_compliance_metadata", [
      nativeToScVal(key, { type: "symbol" }),
    ]);
  }
}
