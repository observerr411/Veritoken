import { Address, nativeToScVal, rpc } from "@stellar/stellar-sdk";
import { BaseContractClient, scVal } from "./base.js";

export class RwaTokenClient extends BaseContractClient {
  constructor(contractId: string, server: rpc.Server, networkPassphrase: string) {
    super(contractId, server, networkPassphrase);
  }

  // ── Read API ─────────────────────────────────────────────────────────────

  async balance(addr: string): Promise<bigint> {
    return scVal<bigint>(
      await this.simulate("balance", [new Address(addr).toScVal()]),
    );
  }

  async totalSupply(): Promise<bigint> {
    return scVal<bigint>(await this.simulate("total_supply", []));
  }

  async allowance(from: string, spender: string): Promise<bigint> {
    return scVal<bigint>(
      await this.simulate("allowance", [
        new Address(from).toScVal(),
        new Address(spender).toScVal(),
      ]),
    );
  }

  async name(): Promise<string> {
    return scVal<string>(await this.simulate("name", []));
  }

  async symbol(): Promise<string> {
    return scVal<string>(await this.simulate("symbol", []));
  }

  async decimals(): Promise<number> {
    return scVal<number>(await this.simulate("decimals", []));
  }

  async assetType(): Promise<string> {
    return scVal<string>(await this.simulate("asset_type", []));
  }

  async kycRegistry(): Promise<string> {
    return scVal<string>(await this.simulate("kyc_registry", []));
  }

  async complianceEngine(): Promise<string> {
    return scVal<string>(await this.simulate("compliance_engine", []));
  }

  async getComplianceMetadata(key: string): Promise<string> {
    return scVal<string>(
      await this.simulate("get_compliance_metadata", [
        nativeToScVal(key, { type: "symbol" }),
      ]),
    );
  }

  // ── Transaction builders (return operation XDR for signing) ───────────────

  buildMintXdr(to: string, amount: bigint): string {
    return this.buildCallXdr("mint", [
      new Address(to).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildTransferXdr(from: string, to: string, amount: bigint): string {
    return this.buildCallXdr("transfer", [
      new Address(from).toScVal(),
      new Address(to).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildTransferFromXdr(
    spender: string,
    from: string,
    to: string,
    amount: bigint,
  ): string {
    return this.buildCallXdr("transfer_from", [
      new Address(spender).toScVal(),
      new Address(from).toScVal(),
      new Address(to).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildApproveXdr(
    from: string,
    spender: string,
    amount: bigint,
    expirationLedger: number,
  ): string {
    return this.buildCallXdr("approve", [
      new Address(from).toScVal(),
      new Address(spender).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
      nativeToScVal(expirationLedger, { type: "u32" }),
    ]);
  }

  buildBurnXdr(from: string, amount: bigint): string {
    return this.buildCallXdr("burn", [
      new Address(from).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildBurnFromXdr(spender: string, from: string, amount: bigint): string {
    return this.buildCallXdr("burn_from", [
      new Address(spender).toScVal(),
      new Address(from).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildSetAdminXdr(newAdmin: string): string {
    return this.buildCallXdr("set_admin", [new Address(newAdmin).toScVal()]);
  }

  buildSetComplianceMetadataXdr(key: string, value: string): string {
    return this.buildCallXdr("set_compliance_metadata", [
      nativeToScVal(key, { type: "symbol" }),
      nativeToScVal(value, { type: "string" }),
    ]);
  }
}
