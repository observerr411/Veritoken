import {
  Contract,
  Account,
  Keypair,
  TransactionBuilder,
  scValToNative,
  xdr,
  rpc,
} from "@stellar/stellar-sdk";

// Dummy source keypair used only for fee-less simulation reads.
// The simulation network ignores the source signature for view calls.
const SIM_KEYPAIR = Keypair.random();
const SIM_ACCOUNT = new Account(SIM_KEYPAIR.publicKey(), "0");

export abstract class BaseContractClient {
  protected readonly contract: Contract;

  constructor(
    protected readonly contractId: string,
    protected readonly server: rpc.Server,
    protected readonly networkPassphrase: string,
  ) {
    this.contract = new Contract(contractId);
  }

  protected async simulate(method: string, args: xdr.ScVal[]): Promise<xdr.ScVal> {
    const tx = new TransactionBuilder(SIM_ACCOUNT, {
      fee: "100",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const result = await this.server.simulateTransaction(tx);

    if (rpc.Api.isSimulationError(result)) {
      throw new Error(`Simulation error calling ${method}: ${result.error}`);
    }
    if (!result.result?.retval) {
      throw new Error(`No return value from ${method}`);
    }
    return result.result.retval;
  }

  protected buildCallXdr(method: string, args: xdr.ScVal[]): string {
    return this.contract.call(method, ...args).toXDR("base64");
  }
}

export function scVal<T>(val: xdr.ScVal): T {
  return scValToNative(val) as T;
}
