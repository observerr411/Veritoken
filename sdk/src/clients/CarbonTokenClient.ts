import { Address, nativeToScVal, rpc } from "@stellar/stellar-sdk";
import { BaseContractClient, scVal } from "./base.js";
import type { ProjectMeta, RetirementReceipt } from "../types.js";

export class CarbonTokenClient extends BaseContractClient {
  constructor(contractId: string, server: rpc.Server, networkPassphrase: string) {
    super(contractId, server, networkPassphrase);
  }

  // ── Read API ─────────────────────────────────────────────────────────────

  async getMeta(): Promise<ProjectMeta> {
    return scVal<ProjectMeta>(await this.simulate("get_meta", []));
  }

  async balance(addr: string): Promise<bigint> {
    return scVal<bigint>(
      await this.simulate("balance", [new Address(addr).toScVal()]),
    );
  }

  async totalSupply(): Promise<bigint> {
    return scVal<bigint>(await this.simulate("total_supply", []));
  }

  async totalRetired(): Promise<bigint> {
    return scVal<bigint>(await this.simulate("total_retired", []));
  }

  async retirementCount(): Promise<number> {
    return scVal<number>(await this.simulate("retirement_count", []));
  }

  async getReceipt(index: number): Promise<RetirementReceipt> {
    return scVal<RetirementReceipt>(
      await this.simulate("get_receipt", [nativeToScVal(index, { type: "u32" })]),
    );
  }

  async getReceipts(start: number, limit: number): Promise<RetirementReceipt[]> {
    return scVal<RetirementReceipt[]>(
      await this.simulate("get_receipts", [
        nativeToScVal(start, { type: "u32" }),
        nativeToScVal(limit, { type: "u32" }),
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

  buildRetireXdr(
    retiree: string,
    amount: bigint,
    beneficiary: string,
    reason: string,
  ): string {
    return this.buildCallXdr("retire", [
      new Address(retiree).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
      nativeToScVal(beneficiary, { type: "string" }),
      nativeToScVal(reason, { type: "string" }),
    ]);
  }

  buildUpdateMetaXdr(meta: ProjectMeta): string {
    return this.buildCallXdr("update_meta", [nativeToScVal(meta)]);
  }
}
