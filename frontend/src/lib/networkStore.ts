import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type Network = "testnet" | "mainnet";

export interface NetworkStore {
  network: Network;
  setNetwork: (network: Network) => void;
}

const STORAGE_KEY = "veritoken-network";

export const useNetworkStore = create<NetworkStore>()(
  persist(
    (set) => ({
      network: (import.meta.env.VITE_STELLAR_NETWORK as Network) ?? "testnet",
      setNetwork: (network: Network) => set({ network }),
    }),
    {
      name: STORAGE_KEY,
      storage: createJSONStorage(() => localStorage),
    }
  )
);

export const getNetworkRpcUrl = (network: Network): string => {
  return network === "mainnet"
    ? "https://mainnet.sorobanrpc.com"
    : "https://soroban-testnet.stellar.org";
};
