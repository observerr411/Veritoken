import { useState, useEffect } from "react";
import { Contract, scValToNative, TransactionBuilder, Account, Operation, xdr } from "@stellar/stellar-sdk";
import { useWallet } from "../lib/wallet";
import { server, CONTRACT_IDS, NETWORK_PASSPHRASE } from "../lib/stellar";
import { PageHeader, Card, Field, Icon } from "../components/ui";
import type { ComplianceRules } from "../types";

// Shape that mirrors the form fields (all strings for controlled inputs)
interface RulesFormState {
  max_transfer_amount: string;
  min_holding_period: string;
  max_holders: string;
  require_same_jurisdiction: boolean;
  paused: boolean;
}

const DEFAULT_RULES: RulesFormState = {
  max_transfer_amount: "0",
  min_holding_period: "0",
  max_holders: "0",
  require_same_jurisdiction: false,
  paused: false,
};

export default function AdminPage() {
  const { connected } = useWallet();

  const [rules, setRules] = useState<RulesFormState>(DEFAULT_RULES);
  const [loading, setLoading] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);

  // ── Fetch current on-chain rules on mount ────────────────────────────────
  useEffect(() => {
    if (!CONTRACT_IDS.complianceEngine) return;

    let cancelled = false;

    async function fetchRules() {
      setLoading(true);
      setFetchError(null);

      try {
        const contract = new Contract(CONTRACT_IDS.complianceEngine);

        // Build a fee-bump-free transaction for simulation only.
        // We use a dummy account because simulation does not require auth.
        const dummyAccount = new Account(
          "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN",
          "0"
        );

        const tx = new TransactionBuilder(dummyAccount, {
          fee: "100",
          networkPassphrase: NETWORK_PASSPHRASE,
        })
          .addOperation(contract.call("get_rules"))
          .setTimeout(30)
          .build();

        const simResult = await server.simulateTransaction(tx);

        if ("error" in simResult && simResult.error) {
          throw new Error(`Simulation error: ${simResult.error}`);
        }

        // Extract the return value from the simulation result.
        const returnVal = (simResult as { result?: { retval: xdr.ScVal } }).result?.retval;
        if (!returnVal) {
          throw new Error("No return value from get_rules simulation");
        }

        const decoded = scValToNative(returnVal) as ComplianceRules;

        if (!cancelled) {
          setRules({
            max_transfer_amount: String(decoded.max_transfer_amount ?? 0),
            min_holding_period: String(decoded.min_holding_period ?? 0),
            max_holders: String(decoded.max_holders ?? 0),
            require_same_jurisdiction: Boolean(decoded.require_same_jurisdiction),
            paused: Boolean(decoded.paused),
          });
        }
      } catch (err) {
        if (!cancelled) {
          setFetchError(
            err instanceof Error ? err.message : "Failed to fetch compliance rules from chain."
          );
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchRules();
    return () => {
      cancelled = true;
    };
  }, []);

  // ── Form submit ──────────────────────────────────────────────────────────
  const handleSaveRules = (e: React.FormEvent) => {
    e.preventDefault();
    if (!connected) return alert("Connect wallet first");
    alert("Would call set_rules() on compliance engine");
  };

  // ── Render ───────────────────────────────────────────────────────────────
  return (
    <div className="form-narrow">
      <PageHeader
        eyebrow="Governance"
        icon={<Icon.admin size={22} />}
        title="Admin Panel"
        description="Configure global compliance rules. Only the contract admin can call these functions."
      />

      {/* Error banner */}
      {fetchError && (
        <div style={styles.errorBanner} role="alert">
          <strong>Failed to load current rules:</strong> {fetchError}
        </div>
      )}

      <Card title="Compliance Rules">
        {loading ? (
          <div style={styles.spinnerWrap} aria-label="Loading compliance rules">
            <span style={styles.spinner} aria-hidden="true" />
            <span className="muted" style={{ fontSize: "0.9rem" }}>
              Loading current rules from chain…
            </span>
          </div>
        ) : (
          <form onSubmit={handleSaveRules}>
            <Field
              label="Max Transfer Amount (0 = unlimited, in stroops)"
              type="number"
              value={rules.max_transfer_amount}
              onChange={(e) => setRules((r) => ({ ...r, max_transfer_amount: e.target.value }))}
            />
            <Field
              label="Min Holding Period (seconds, 0 = none)"
              type="number"
              value={rules.min_holding_period}
              onChange={(e) => setRules((r) => ({ ...r, min_holding_period: e.target.value }))}
            />
            <Field
              label="Max Holders (0 = unlimited)"
              type="number"
              value={rules.max_holders}
              onChange={(e) => setRules((r) => ({ ...r, max_holders: e.target.value }))}
            />
            <label style={styles.checkboxRow}>
              <input
                type="checkbox"
                style={{ width: "auto" }}
                checked={rules.require_same_jurisdiction}
                onChange={(e) =>
                  setRules((r) => ({ ...r, require_same_jurisdiction: e.target.checked }))
                }
              />
              <span style={{ fontSize: "0.875rem", color: "var(--text)" }}>
                Require same jurisdiction for transfers
              </span>
            </label>
            <button type="submit" className="btn-block">
              Save Rules
            </button>
          </form>
        )}
      </Card>

      <Card
        title="Emergency Controls"
        subtitle="Pause halts every transfer across all asset tokens"
        style={{ marginTop: "1.25rem" }}
      >
        <div style={{ display: "flex", gap: "1rem" }}>
          <button
            onClick={() => alert("Would call pause() on compliance engine")}
            className="btn-danger"
            style={{ flex: 1 }}
          >
            <Icon.bolt
              size={15}
              style={{ display: "inline", verticalAlign: "-2px", marginRight: 6 }}
            />
            Pause All Transfers
          </button>
          <button
            onClick={() => alert("Would call unpause() on compliance engine")}
            className="btn-success"
            style={{ flex: 1 }}
          >
            Unpause Transfers
          </button>
        </div>
      </Card>
    </div>
  );
}

const SPINNER_KEYFRAMES = `
@keyframes vt-spin {
  to { transform: rotate(360deg); }
}
`;

// Inject keyframes once (safe to call multiple times – deduped by id)
if (typeof document !== "undefined") {
  const id = "vt-spinner-style";
  if (!document.getElementById(id)) {
    const style = document.createElement("style");
    style.id = id;
    style.textContent = SPINNER_KEYFRAMES;
    document.head.appendChild(style);
  }
}

const styles: Record<string, React.CSSProperties> = {
  checkboxRow: {
    display: "flex",
    alignItems: "center",
    gap: "0.6rem",
    margin: "0.25rem 0 1.1rem",
    cursor: "pointer",
  },
  spinnerWrap: {
    display: "flex",
    alignItems: "center",
    gap: "0.75rem",
    padding: "1.5rem 0",
  },
  spinner: {
    display: "inline-block",
    width: 20,
    height: 20,
    borderRadius: "50%",
    border: "2.5px solid var(--border)",
    borderTopColor: "var(--accent-2)",
    animation: "vt-spin 0.7s linear infinite",
    flexShrink: 0,
  },
  errorBanner: {
    marginBottom: "1.25rem",
    padding: "0.85rem 1rem",
    borderRadius: 10,
    background: "color-mix(in srgb, #ef4444 12%, transparent)",
    border: "1px solid color-mix(in srgb, #ef4444 35%, transparent)",
    color: "#ef4444",
    fontSize: "0.875rem",
    lineHeight: 1.5,
  },
};
