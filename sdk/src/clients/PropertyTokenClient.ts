import { Address, nativeToScVal, rpc } from "@stellar/stellar-sdk";
import { BaseContractClient, scVal } from "./base.js";
import type { PropertyMeta } from "../types.js";

export class PropertyTokenClient extends BaseContractClient {
  constructor(contractId: string, server: rpc.Server, networkPassphrase: string) {
    super(contractId, server, networkPassphrase);
  }

  // ── Read API ─────────────────────────────────────────────────────────────

  async getMeta(): Promise<PropertyMeta> {
    return scVal<PropertyMeta>(await this.simulate("get_meta", []));
  }

  async balance(addr: string): Promise<bigint> {
    return scVal<bigint>(
      await this.simulate("balance", [new Address(addr).toScVal()]),
    );
  }

  async totalShares(): Promise<bigint> {
    return scVal<bigint>(await this.simulate("total_shares", []));
  }

  async pendingDividend(holder: string): Promise<bigint> {
    return scVal<bigint>(
      await this.simulate("pending_dividend", [new Address(holder).toScVal()]),
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

  buildMintXdr(to: string, shares: bigint): string {
    return this.buildCallXdr("mint", [
      new Address(to).toScVal(),
      nativeToScVal(shares, { type: "i128" }),
    ]);
  }

  buildTransferXdr(from: string, to: string, shares: bigint): string {
    return this.buildCallXdr("transfer", [
      new Address(from).toScVal(),
      new Address(to).toScVal(),
      nativeToScVal(shares, { type: "i128" }),
    ]);
  }

  buildDepositDividendXdr(amount: bigint): string {
    return this.buildCallXdr("deposit_dividend", [
      nativeToScVal(amount, { type: "i128" }),
    ]);
  }

  buildClaimDividendXdr(holder: string): string {
    return this.buildCallXdr("claim_dividend", [new Address(holder).toScVal()]);
  }
}
