import { useState } from "react";
import { useWallet } from "../lib/wallet";
import { contracts } from "../lib/contracts";
import { PageHeader, Card, Field, Icon } from "../components/ui";
import type { RetirementReceipt } from "../types";

const PAGE_SIZE = 10;

// ── Loading spinner ───────────────────────────────────────────────────────────

function Spinner() {
  return (
    <span
      role="status"
      aria-label="Loading"
      style={{
        display: "inline-block",
        width: 16,
        height: 16,
        border: "2px solid currentColor",
        borderTopColor: "transparent",
        borderRadius: "50%",
        animation: "spin 0.7s linear infinite",
        verticalAlign: "middle",
        marginRight: 6,
      }}
    />
  );
}

// ── Error banner ──────────────────────────────────────────────────────────────

function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div
      role="alert"
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "flex-start",
        gap: "0.5rem",
        padding: "0.75rem 1rem",
        marginBottom: "1rem",
        borderRadius: 10,
        background: "var(--error-soft, #fee2e2)",
        border: "1px solid var(--error, #f87171)",
        color: "var(--error-text, #991b1b)",
        fontSize: "0.875rem",
      }}
    >
      <span>{message}</span>
      <button
        onClick={onDismiss}
        aria-label="Dismiss error"
        style={{
          background: "none",
          border: "none",
          cursor: "pointer",
          padding: 0,
          lineHeight: 1,
          fontSize: "1rem",
          color: "inherit",
          flexShrink: 0,
        }}
      >
        ✕
      </button>
    </div>
  );
}

// ── Receipt card ──────────────────────────────────────────────────────────────

function ReceiptCard({
  receipt,
  index,
  highlight = false,
}: {
  receipt: RetirementReceipt;
  index: number;
  highlight?: boolean;
}) {
  const amount =
    typeof receipt.amount === "bigint"
      ? Number(receipt.amount)
      : (receipt.amount as number);

  return (
    <div
      style={{
        display: "flex",
        gap: "0.75rem",
        padding: "0.65rem 0",
        borderBottom: "1px solid var(--border)",
        background: highlight ? "var(--accent-soft)" : undefined,
        borderRadius: highlight ? 8 : undefined,
        paddingLeft: highlight ? "0.5rem" : undefined,
      }}
    >
      <div
        style={{
          minWidth: 36,
          color: "var(--muted)",
          fontSize: "0.8rem",
          paddingTop: 2,
        }}
      >
        #{index}
      </div>
      <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "0.15rem" }}>
        <div style={{ fontWeight: 500 }}>
          {amount.toLocaleString()} tCO₂e
          {receipt.beneficiary ? ` — ${receipt.beneficiary}` : ""}
        </div>
        <div className="muted" style={{ fontSize: "0.78rem" }}>
          {receipt.retiree} ·{" "}
          {new Date(
            (typeof receipt.timestamp === "bigint"
              ? Number(receipt.timestamp)
              : receipt.timestamp) * 1000
          ).toLocaleDateString()}
        </div>
        {receipt.retirement_reason && (
          <div className="muted" style={{ fontSize: "0.78rem" }}>
            {receipt.retirement_reason}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Main page component ───────────────────────────────────────────────────────

export default function CarbonPage() {
  const { connected, address, signTx } = useWallet();
  const [tab, setTab] = useState<"issue" | "retire" | "receipts">("issue");

  // ── Mint form ──────────────────────────────────────────────────────────────
  const [mintTo, setMintTo] = useState("");
  const [mintAmount, setMintAmount] = useState("");
  const [mintLoading, setMintLoading] = useState(false);
  const [mintError, setMintError] = useState<string | null>(null);
  const [mintSuccess, setMintSuccess] = useState(false);

  // ── Transfer form ─────────────────────────────────────────────────────────
  const [transferTo, setTransferTo] = useState("");
  const [transferAmount, setTransferAmount] = useState("");
  const [transferLoading, setTransferLoading] = useState(false);
  const [transferError, setTransferError] = useState<string | null>(null);
  const [transferSuccess, setTransferSuccess] = useState(false);

  // ── Retire form ───────────────────────────────────────────────────────────
  const [retireAmount, setRetireAmount] = useState("");
  const [retireBeneficiary, setRetireBeneficiary] = useState("");
  const [retireReason, setRetireReason] = useState("");
  const [retireLoading, setRetireLoading] = useState(false);
  const [retireError, setRetireError] = useState<string | null>(null);
  const [lastReceipt, setLastReceipt] = useState<RetirementReceipt | null>(null);

  // ── Receipts ──────────────────────────────────────────────────────────────
  const [receipts, setReceipts] = useState<RetirementReceipt[]>([]);
  const [totalCount, setTotalCount] = useState<number | null>(null);
  const [page, setPage] = useState(0);
  const [receiptsLoading, setReceiptsLoading] = useState(false);
  const [receiptsError, setReceiptsError] = useState<string | null>(null);

  // ── Handlers ──────────────────────────────────────────────────────────────

  const handleMint = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!connected || !address) {
      setMintError("Connect your wallet first.");
      return;
    }
    setMintLoading(true);
    setMintError(null);
    setMintSuccess(false);
    try {
      await contracts.carbonToken.mint(
        mintTo || address,
        BigInt(mintAmount),
        signTx
      );
      setMintSuccess(true);
      setMintAmount("");
      setMintTo("");
    } catch (err) {
      setMintError(err instanceof Error ? err.message : String(err));
    } finally {
      setMintLoading(false);
    }
  };

  const handleTransfer = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!connected || !address) {
      setTransferError("Connect your wallet first.");
      return;
    }
    setTransferLoading(true);
    setTransferError(null);
    setTransferSuccess(false);
    try {
      await contracts.carbonToken.transfer(
        address,
        transferTo,
        BigInt(transferAmount),
        signTx
      );
      setTransferSuccess(true);
      setTransferAmount("");
      setTransferTo("");
    } catch (err) {
      setTransferError(err instanceof Error ? err.message : String(err));
    } finally {
      setTransferLoading(false);
    }
  };

  const handleRetire = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!connected || !address) {
      setRetireError("Connect your wallet first.");
      return;
    }
    setRetireLoading(true);
    setRetireError(null);
    setLastReceipt(null);
    try {
      const receipt = await contracts.carbonToken.retire(
        address,
        BigInt(retireAmount),
        retireBeneficiary,
        retireReason,
        signTx
      );
      setLastReceipt(receipt);
      setRetireAmount("");
      setRetireBeneficiary("");
      setRetireReason("");
    } catch (err) {
      setRetireError(err instanceof Error ? err.message : String(err));
    } finally {
      setRetireLoading(false);
    }
  };

  const loadReceipts = async (targetPage: number) => {
    if (!address) {
      setReceiptsError("Connect your wallet to load receipts.");
      return;
    }
    setReceiptsLoading(true);
    setReceiptsError(null);
    try {
      const count = await contracts.carbonToken.retirementCount(address);
      setTotalCount(count);
      const start = targetPage * PAGE_SIZE;
      const fetched = await contracts.carbonToken.getReceipts(
        start,
        PAGE_SIZE,
        address
      );
      setReceipts(fetched);
      setPage(targetPage);
    } catch (err) {
      setReceiptsError(err instanceof Error ? err.message : String(err));
    } finally {
      setReceiptsLoading(false);
    }
  };

  const handleTabReceipts = () => {
    setTab("receipts");
    if (receipts.length === 0 && totalCount === null) {
      loadReceipts(0);
    }
  };

  const totalPages =
    totalCount !== null ? Math.ceil(totalCount / PAGE_SIZE) : null;

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="form-narrow">
      <style>{`
        @keyframes spin { to { transform: rotate(360deg); } }
      `}</style>

      <PageHeader
        eyebrow="Asset Module"
        icon={<Icon.carbon size={22} />}
        title="Carbon Credit Token"
        description="Issue verified carbon credits (1 token = 1 tonne CO₂e) and retire them with permanent on-chain receipts."
      />

      <div style={styles.tabs}>
        <button
          onClick={() => setTab("issue")}
          className={tab === "issue" ? "" : "btn-ghost"}
          style={styles.tab}
        >
          Issue Credits
        </button>
        <button
          onClick={() => setTab("retire")}
          className={tab === "retire" ? "" : "btn-ghost"}
          style={styles.tab}
        >
          Retire Credits
        </button>
        <button
          onClick={handleTabReceipts}
          className={tab === "receipts" ? "" : "btn-ghost"}
          style={styles.tab}
        >
          Receipts
        </button>
      </div>

      {/* ── Issue tab ───────────────────────────────────────────────────── */}
      {tab === "issue" && (
        <Card>
          {mintError && (
            <ErrorBanner message={mintError} onDismiss={() => setMintError(null)} />
          )}
          {mintSuccess && (
            <div
              role="status"
              style={{
                padding: "0.65rem 1rem",
                marginBottom: "1rem",
                borderRadius: 10,
                background: "var(--success-soft, #dcfce7)",
                border: "1px solid var(--success, #4ade80)",
                color: "var(--success-text, #166534)",
                fontSize: "0.875rem",
              }}
            >
              Credits issued successfully.
            </div>
          )}
          <form onSubmit={handleMint}>
            <Field
              label="Recipient Address"
              value={mintTo}
              onChange={(e) => setMintTo(e.target.value)}
              placeholder={address ?? "G…"}
            />
            <Field
              label="Credits to Mint (tonnes CO₂e)"
              type="number"
              value={mintAmount}
              onChange={(e) => setMintAmount(e.target.value)}
              required
            />
            <button
              type="submit"
              className="btn-block"
              style={{ marginTop: "0.75rem" }}
              disabled={mintLoading}
            >
              {mintLoading && <Spinner />}
              {mintLoading ? "Issuing…" : "Issue Carbon Credits"}
            </button>
          </form>

          {/* Transfer sub-section */}
          <hr style={{ margin: "1.5rem 0", borderColor: "var(--border)" }} />
          <h3 style={{ fontSize: "0.95rem", fontWeight: 700, marginBottom: "0.75rem" }}>
            Transfer Credits
          </h3>
          {transferError && (
            <ErrorBanner
              message={transferError}
              onDismiss={() => setTransferError(null)}
            />
          )}
          {transferSuccess && (
            <div
              role="status"
              style={{
                padding: "0.65rem 1rem",
                marginBottom: "1rem",
                borderRadius: 10,
                background: "var(--success-soft, #dcfce7)",
                border: "1px solid var(--success, #4ade80)",
                color: "var(--success-text, #166534)",
                fontSize: "0.875rem",
              }}
            >
              Transfer sent successfully.
            </div>
          )}
          <form onSubmit={handleTransfer}>
            <Field
              label="Recipient Address"
              value={transferTo}
              onChange={(e) => setTransferTo(e.target.value)}
              placeholder="G…"
              required
            />
            <Field
              label="Amount (tonnes CO₂e)"
              type="number"
              value={transferAmount}
              onChange={(e) => setTransferAmount(e.target.value)}
              required
            />
            <button
              type="submit"
              className="btn-block btn-ghost"
              style={{ marginTop: "0.75rem" }}
              disabled={transferLoading}
            >
              {transferLoading && <Spinner />}
              {transferLoading ? "Transferring…" : "Transfer Credits"}
            </button>
          </form>
        </Card>
      )}

      {/* ── Retire tab ──────────────────────────────────────────────────── */}
      {tab === "retire" && (
        <Card>
          {retireError && (
            <ErrorBanner
              message={retireError}
              onDismiss={() => setRetireError(null)}
            />
          )}
          <form onSubmit={handleRetire}>
            <Field
              label="Amount to Retire (tonnes CO₂e)"
              type="number"
              value={retireAmount}
              onChange={(e) => setRetireAmount(e.target.value)}
              required
            />
            <Field
              label="Beneficiary Name"
              value={retireBeneficiary}
              onChange={(e) => setRetireBeneficiary(e.target.value)}
              placeholder="Acme Corp 2024 offset"
            />
            <Field
              label="Retirement Reason"
              value={retireReason}
              onChange={(e) => setRetireReason(e.target.value)}
              placeholder="Annual Scope 1 offset"
            />
            <p
              className="muted"
              style={{ fontSize: "0.78rem", margin: "0.25rem 0 0.9rem" }}
            >
              Retirement is permanent — credits are burned and cannot be re-issued.
            </p>
            <button
              type="submit"
              className="btn-success btn-block"
              disabled={retireLoading}
            >
              {retireLoading && <Spinner />}
              {retireLoading ? "Retiring…" : "Retire Credits (Permanent)"}
            </button>
          </form>

          {/* Receipt result panel */}
          {lastReceipt && (
            <div
              style={{
                marginTop: "1.25rem",
                padding: "1rem",
                borderRadius: 12,
                background: "var(--accent-soft)",
                border: "1px solid var(--border)",
              }}
            >
              <div
                style={{ fontWeight: 700, marginBottom: "0.5rem", fontSize: "0.95rem" }}
              >
                Retirement Receipt
              </div>
              <dl style={styles.receipt}>
                <dt>Retiree</dt>
                <dd style={styles.mono}>{lastReceipt.retiree}</dd>
                <dt>Amount</dt>
                <dd>
                  {(typeof lastReceipt.amount === "bigint"
                    ? Number(lastReceipt.amount)
                    : lastReceipt.amount
                  ).toLocaleString()}{" "}
                  tCO₂e
                </dd>
                {lastReceipt.beneficiary && (
                  <>
                    <dt>Beneficiary</dt>
                    <dd>{lastReceipt.beneficiary}</dd>
                  </>
                )}
                {lastReceipt.retirement_reason && (
                  <>
                    <dt>Reason</dt>
                    <dd>{lastReceipt.retirement_reason}</dd>
                  </>
                )}
                <dt>Timestamp</dt>
                <dd>
                  {new Date(
                    (typeof lastReceipt.timestamp === "bigint"
                      ? Number(lastReceipt.timestamp)
                      : lastReceipt.timestamp) * 1000
                  ).toLocaleString()}
                </dd>
              </dl>
            </div>
          )}
        </Card>
      )}

      {/* ── Receipts tab ────────────────────────────────────────────────── */}
      {tab === "receipts" && (
        <Card>
          <div style={styles.receiptsHeader}>
            <span style={{ fontWeight: 600 }}>
              Retirement Receipts
              {totalCount !== null && (
                <span
                  className="muted"
                  style={{ fontWeight: 400, marginLeft: "0.4rem" }}
                >
                  ({totalCount} total)
                </span>
              )}
            </span>
            <button
              className="btn-ghost"
              style={{ fontSize: "0.8rem" }}
              onClick={() => loadReceipts(page)}
              disabled={receiptsLoading}
            >
              {receiptsLoading ? (
                <>
                  <Spinner />
                  Loading…
                </>
              ) : (
                "Refresh"
              )}
            </button>
          </div>

          {receiptsError && (
            <ErrorBanner
              message={receiptsError}
              onDismiss={() => setReceiptsError(null)}
            />
          )}

          {receipts.length === 0 && !receiptsLoading && !receiptsError && (
            <p
              className="muted"
              style={{ fontSize: "0.85rem", margin: "1rem 0" }}
            >
              No receipts loaded. Connect your wallet and click Refresh.
            </p>
          )}

          {receipts.map((r, i) => (
            <ReceiptCard
              key={page * PAGE_SIZE + i}
              receipt={r}
              index={page * PAGE_SIZE + i}
            />
          ))}

          {totalPages !== null && totalPages > 1 && (
            <div style={styles.pagination}>
              <button
                className="btn-ghost"
                onClick={() => loadReceipts(page - 1)}
                disabled={page === 0 || receiptsLoading}
              >
                ← Prev
              </button>
              <span className="muted" style={{ fontSize: "0.85rem" }}>
                Page {page + 1} / {totalPages}
              </span>
              <button
                className="btn-ghost"
                onClick={() => loadReceipts(page + 1)}
                disabled={page >= totalPages - 1 || receiptsLoading}
              >
                Next →
              </button>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  tabs: {
    display: "inline-flex",
    gap: "0.35rem",
    padding: "0.3rem",
    marginBottom: "1.5rem",
    background: "var(--surface-2)",
    border: "1px solid var(--border)",
    borderRadius: 12,
  },
  tab: { boxShadow: "none" },
  receiptsHeader: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "0.75rem",
  },
  pagination: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginTop: "1rem",
  },
  receipt: {
    display: "grid",
    gridTemplateColumns: "max-content 1fr",
    gap: "0.25rem 0.75rem",
    fontSize: "0.85rem",
    margin: 0,
  },
  mono: {
    fontFamily: "monospace",
    fontSize: "0.75rem",
    wordBreak: "break-all" as const,
  },
};
