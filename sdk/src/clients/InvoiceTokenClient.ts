import { Address, nativeToScVal, rpc } from "@stellar/stellar-sdk";
import { BaseContractClient, scVal } from "./base.js";
import type { InvoiceMeta } from "../types.js";

export class InvoiceTokenClient extends BaseContractClient {
  constructor(contractId: string, server: rpc.Server, networkPassphrase: string) {
    super(contractId, server, networkPassphrase);
  }

  // ── Read API ─────────────────────────────────────────────────────────────

  async getMeta(): Promise<InvoiceMeta> {
    return scVal<InvoiceMeta>(await this.simulate("get_meta", []));
  }

  async balance(addr: string): Promise<bigint> {
    return scVal<bigint>(
      await this.simulate("balance", [new Address(addr).toScVal()]),
    );
  }

  async totalSupply(): Promise<bigint> {
    return scVal<bigint>(await this.simulate("total_supply", []));
  }

  async isSettled(): Promise<boolean> {
    return scVal<boolean>(await this.simulate("is_settled", []));
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

  // ── Transaction builders (return operation XDR for signing) ───────────────

  buildIssueXdr(to: string, amount: bigint): string {
    return this.buildCallXdr("issue", [
      new Address(to).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildSettleXdr(): string {
    return this.buildCallXdr("settle", []);
  }

  buildRedeemXdr(from: string, amount: bigint): string {
    return this.buildCallXdr("redeem", [
      new Address(from).toScVal(),
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

  buildUpdateMetaXdr(meta: InvoiceMeta): string {
    return this.buildCallXdr("update_meta", [nativeToScVal(meta)]);
  }
}
