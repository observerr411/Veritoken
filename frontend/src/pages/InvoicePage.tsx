import { useState, useEffect } from "react";
import { useWallet } from "../lib/wallet";
import { CONTRACT_IDS, fetchContractEvents } from "../lib/stellar";
import { PageHeader, Card, Field, Icon } from "../components/ui";
import WalletGuard from "../components/WalletGuard";
import { useToast } from "../lib/toast";
import type { ContractEvent } from "../types";

export default function InvoicePage() {
  const { } = useWallet();
  const { addToast } = useToast();
  const [form, setForm] = useState({
    invoice_id: "",
    issuer: "",
    debtor: "",
    face_value_usd: "",
    discount_rate_bps: "0",
    due_date: "",
    currency: "USD",
    ipfs_doc_hash: "",
  });
  const [events, setEvents] = useState<ContractEvent[]>([]);
  const [eventsLoading, setEventsLoading] = useState(false);

  useEffect(() => {
    if (!CONTRACT_IDS.invoiceToken) return;
    setEventsLoading(true);
    fetchContractEvents(CONTRACT_IDS.invoiceToken, 10)
      .then(setEvents)
      .catch(() => {})
      .finally(() => setEventsLoading(false));
  }, []);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setForm((f) => ({ ...f, [e.target.name]: e.target.value }));
  };

  const handleIssue = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      addToast(`Invoice ${form.invoice_id} tokenized successfully.`, "success");
      setForm({ invoice_id: "", issuer: "", debtor: "", face_value_usd: "", discount_rate_bps: "0", due_date: "", currency: "USD", ipfs_doc_hash: "" });
    } catch (err) {
      addToast(err instanceof Error ? err.message : "Failed to tokenize invoice.", "error");
    }
  };

  return (
    <div className="form-narrow">
      <PageHeader
        eyebrow="Asset Module"
        icon={<Icon.invoice size={22} />}
        title="Invoice Token"
        description="Tokenize an accounts-receivable invoice. Each token unit represents one stroop (10⁻⁷ USD) of face value."
      />
      <WalletGuard>
        <Card>
          <form onSubmit={handleIssue}>
            <Field label="Invoice ID" name="invoice_id" value={form.invoice_id} onChange={handleChange} required />
            <Field label="Issuer (company name)" name="issuer" value={form.issuer} onChange={handleChange} required />
            <Field label="Debtor (buyer name)" name="debtor" value={form.debtor} onChange={handleChange} required />
            <Field label="Face Value (USD)" name="face_value_usd" type="number" value={form.face_value_usd} onChange={handleChange} required />
            <Field label="Discount Rate (bps)" name="discount_rate_bps" type="number" value={form.discount_rate_bps} onChange={handleChange} />
            <Field label="Due Date" name="due_date" type="date" value={form.due_date} onChange={handleChange} required />
            <Field label="Currency" name="currency" value={form.currency} onChange={handleChange} />
            <Field label="IPFS Document Hash" name="ipfs_doc_hash" value={form.ipfs_doc_hash} onChange={handleChange} placeholder="bafyrei…" />
            <button type="submit" className="btn-block" style={{ marginTop: "0.75rem" }}>
              Tokenize Invoice
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
              <th style={th}>Type</th><th style={th}>Amount</th><th style={th}>Counterparty</th><th style={th}>Time</th>
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
