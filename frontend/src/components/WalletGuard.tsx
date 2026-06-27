import type { ReactNode } from "react";
import { useWallet } from "../lib/wallet";
import { Card } from "./ui";

export default function WalletGuard({ children }: { children: ReactNode }) {
  const { connected, connect } = useWallet();

  if (!connected) {
    return (
      <div style={{ display: "flex", justifyContent: "center", marginTop: "3rem" }}>
        <Card style={{ textAlign: "center", maxWidth: 380 }}>
          <p style={{ marginBottom: "1.25rem", fontSize: "0.95rem" }}>
            Connect your Freighter wallet to continue
          </p>
          <button className="btn-block" onClick={() => connect().catch(() => {})}>
            Connect Wallet
          </button>
        </Card>
      </div>
    );
  }

  return <>{children}</>;
}
