import { useState, useEffect } from "react";
import { useWallet } from "../lib/wallet";
import { CONTRACT_IDS, fetchContractEvents } from "../lib/stellar";
import { PageHeader, Card, Field, Select, Icon } from "../components/ui";
import WalletGuard from "../components/WalletGuard";
import { useToast } from "../lib/toast";
import type { ContractEvent } from "../types";

export default function KycPage() {
  const { } = useWallet();
  const { addToast } = useToast();
  const [lookup, setLookup] = useState("");
  const [approveForm, setApproveForm] = useState({
    subject: "",
    tier: "0",
    jurisdiction: "",
    expiry_days: "365",
  });
  const [events, setEvents] = useState<ContractEvent[]>([]);
  const [eventsLoading, setEventsLoading] = useState(false);

  const set = (k: string) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
    setApproveForm((f) => ({ ...f, [k]: e.target.value }));

  useEffect(() => {
    if (!CONTRACT_IDS.kycRegistry) return;
    setEventsLoading(true);
    fetchContractEvents(CONTRACT_IDS.kycRegistry, 10)
      .then(setEvents)
      .catch(() => {})
      .finally(() => setEventsLoading(false));
  }, []);

  const handleLookup = (e: React.FormEvent) => {
    e.preventDefault();
    addToast(`Querying KYC status for ${lookup}`, "info");
  };

  const handleApprove = (e: React.FormEvent) => {
    e.preventDefault();
    addToast(`KYC approved for ${approveForm.subject} at tier ${approveForm.tier}`, "success");
    setApproveForm({ subject: "", tier: "0", jurisdiction: "", expiry_days: "365" });
  };

  return (
    <div className="form-narrow">
      <PageHeader
        eyebrow="Compliance"
        icon={<Icon.kyc size={22} />}
        title="KYC Registry"
        description="Manage investor KYC approvals. Only authorized verifiers can approve or revoke status — every token transfer is gated by this registry."
      />

      <Card title="Check KYC Status">
        <form onSubmit={handleLookup} style={{ display: "flex", gap: "0.75rem" }}>
          <input
            placeholder="Stellar address (G…)"
            value={lookup}
            onChange={(e) => setLookup(e.target.value)}
            style={{ flex: 1 }}
          />
          <button type="submit">Lookup</button>
        </form>
      </Card>

      <WalletGuard>
        <Card title="Approve KYC" subtitle="Verifier only" style={{ marginTop: "1.25rem" }}>
          <form onSubmit={handleApprove}>
            <Field label="Subject Address" value={approveForm.subject} onChange={set("subject")} required placeholder="G…" />
            <Select
              label="KYC Tier"
              value={approveForm.tier}
              onChange={set("tier")}
              options={[
                { value: "0", label: "0 — Basic" },
                { value: "1", label: "1 — Accredited Investor" },
                { value: "2", label: "2 — Institutional" },
              ]}
            />
            <Field label="Jurisdiction" value={approveForm.jurisdiction} onChange={set("jurisdiction")} required placeholder="US, EU, NG …" />
            <Field label="Validity (days)" type="number" value={approveForm.expiry_days} onChange={set("expiry_days")} />
            <button type="submit" className="btn-success btn-block" style={{ marginTop: "0.5rem" }}>
              Approve KYC
            </button>
          </form>
        </Card>
      </WalletGuard>

      <RecentTransactions events={events} loading={eventsLoading} />
    </div>
  );
}

function RecentTransactions({ events, loading }: { events: ContractEvent[]; loading: boolean }) {
  return (
    <Card title="Recent Transactions" style={{ marginTop: "1.25rem" }}>
      {loading ? (
        <p className="muted" style={{ fontSize: "0.875rem" }}>Loading…</p>
      ) : events.length === 0 ? (
        <p className="muted" style={{ fontSize: "0.875rem" }}>No recent events found.</p>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.82rem" }}>
          <thead>
            <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left" }}>
              <th style={th}>Type</th>
              <th style={th}>Amount</th>
              <th style={th}>Counterparty</th>
              <th style={th}>Time</th>
            </tr>
          </thead>
          <tbody>
            {events.map((ev, i) => (
              <tr key={i} style={{ borderBottom: "1px solid var(--border)" }}>
                <td style={td}>{ev.type}</td>
                <td style={td}>{ev.amount}</td>
                <td style={{ ...td, fontFamily: "monospace", maxWidth: 140, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{ev.counterparty}</td>
                <td style={td}>{ev.timestamp}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Card>
  );
}

const th: React.CSSProperties = { padding: "0.4rem 0.5rem", fontWeight: 600, color: "var(--muted)" };
const td: React.CSSProperties = { padding: "0.4rem 0.5rem" };
