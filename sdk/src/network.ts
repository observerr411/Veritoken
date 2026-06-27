import { Networks, rpc } from "@stellar/stellar-sdk";
import type { Network } from "./types.js";

export const RPC_URLS: Record<Network, string> = {
  testnet: "https://soroban-testnet.stellar.org",
  mainnet: "https://mainnet.sorobanrpc.com",
};

export const NETWORK_PASSPHRASES: Record<Network, string> = {
  testnet: Networks.TESTNET,
  mainnet: Networks.PUBLIC,
};

export function createServer(network: Network): rpc.Server {
  return new rpc.Server(RPC_URLS[network], { allowHttp: false });
}
