import { Address, nativeToScVal, rpc } from "@stellar/stellar-sdk";
import { BaseContractClient, scVal } from "./base.js";
import type { ComplianceRules } from "../types.js";

export class ComplianceEngineClient extends BaseContractClient {
  constructor(contractId: string, server: rpc.Server, networkPassphrase: string) {
    super(contractId, server, networkPassphrase);
  }

  // ── Read API ─────────────────────────────────────────────────────────────

  async getRules(): Promise<ComplianceRules> {
    return scVal<ComplianceRules>(await this.simulate("get_rules", []));
  }

  async isBlocklisted(addr: string): Promise<boolean> {
    return scVal<boolean>(
      await this.simulate("is_blocklisted", [new Address(addr).toScVal()]),
    );
  }

  async canTransfer(from: string, to: string, amount: bigint): Promise<boolean> {
    return scVal<boolean>(
      await this.simulate("can_transfer", [
        new Address(from).toScVal(),
        new Address(to).toScVal(),
        nativeToScVal(amount, { type: "i128" }),
      ]),
    );
  }

  async holderCount(): Promise<number> {
    return scVal<number>(await this.simulate("holder_count", []));
  }

  // ── Transaction builders (return operation XDR for signing) ───────────────

  buildSetRulesXdr(rules: ComplianceRules): string {
    return this.buildCallXdr("set_rules", [nativeToScVal(rules)]);
  }

  buildAddToBlocklistXdr(addr: string): string {
    return this.buildCallXdr("add_to_blocklist", [new Address(addr).toScVal()]);
  }

  buildRemoveFromBlocklistXdr(addr: string): string {
    return this.buildCallXdr("remove_from_blocklist", [new Address(addr).toScVal()]);
  }

  buildPauseXdr(): string {
    return this.buildCallXdr("pause", []);
  }

  buildUnpauseXdr(): string {
    return this.buildCallXdr("unpause", []);
  }
}
