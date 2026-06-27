import { useState, useEffect } from "react";
import { Link } from "react-router-dom";
import { Icon } from "../components/ui";
import { contracts } from "../lib/contracts/index";
import { CONTRACT_IDS } from "../lib/stellar";

const CARDS = [
  {
    title: "Invoice Tokens",
    description: "Tokenize accounts-receivable invoices and trade fractional invoice claims on Stellar.",
    href: "/invoices",
    icon: <Icon.invoice size={22} />,
    gradient: "linear-gradient(135deg, #6366f1, #818cf8)",
  },
  {
    title: "Property Shares",
    description: "Fractional real estate ownership with pro-rata dividend distribution built in.",
    href: "/property",
    icon: <Icon.property size={22} />,
    gradient: "linear-gradient(135deg, #10b981, #34d399)",
  },
  {
    title: "Carbon Credits",
    description: "Issue and retire verified carbon credits with immutable on-chain receipts.",
    href: "/carbon",
    icon: <Icon.carbon size={22} />,
    gradient: "linear-gradient(135deg, #f59e0b, #fbbf24)",
  },
  {
    title: "KYC Registry",
    description: "Manage investor verification, tiers, and jurisdictions from a single registry.",
    href: "/kyc",
    icon: <Icon.kyc size={22} />,
    gradient: "linear-gradient(135deg, #22d3ee, #38bdf8)",
  },
];

const STATS = [
  { label: "Contract suite", value: "6", sub: "Soroban contracts" },
  { label: "Compliance checks", value: "2", sub: "per transfer" },
  { label: "Asset templates", value: "3", sub: "ready to fork" },
  { label: "Test coverage", value: "40", sub: "passing tests" },
];

interface TokenInfo {
  assetType: string;
  kycRegistry: string;
  complianceEngine: string;
  legalEntity: string;
  governingLaw: string;
  isin: string;
}

function TokenInfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.2rem" }}>
      <span className="muted" style={{ fontSize: "0.75rem", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em" }}>{label}</span>
      <span style={{ fontSize: "0.875rem", fontFamily: mono ? "monospace" : undefined, wordBreak: "break-all" }}>{value}</span>
    </div>
  );
}

export default function Dashboard() {
  const [tokenInfo, setTokenInfo] = useState<TokenInfo | null>(null);
  const [tokenInfoLoading, setTokenInfoLoading] = useState(false);
  const [tokenInfoError, setTokenInfoError] = useState<string | null>(null);

  useEffect(() => {
    if (!CONTRACT_IDS.rwaToken) return;
    setTokenInfoLoading(true);
    setTokenInfoError(null);
    Promise.all([
      contracts.rwa.assetType(),
      contracts.rwa.kycRegistry(),
      contracts.rwa.complianceEngine(),
      contracts.rwa.getComplianceMetadata("legal_entity"),
      contracts.rwa.getComplianceMetadata("governing_law"),
      contracts.rwa.getComplianceMetadata("isin"),
    ])
      .then(([assetType, kycRegistry, complianceEngine, legalEntity, governingLaw, isin]) => {
        setTokenInfo({ assetType, kycRegistry, complianceEngine, legalEntity, governingLaw, isin });
      })
      .catch((err: unknown) => {
        setTokenInfoError(err instanceof Error ? err.message : "Failed to load token info");
      })
      .finally(() => setTokenInfoLoading(false));
  }, []);

  return (
    <div>
      {/* Hero */}
      <section style={styles.hero}>
        <span className="eyebrow">
          <Icon.bolt size={13} /> RWA Tokenization · Stellar
        </span>
        <h1 style={styles.heroTitle}>
          <span className="text-gradient">Real-world assets,</span>
          <br />
          compliant from the first block.
        </h1>
        <p className="muted" style={styles.heroSub}>
          Veritoken is a plug-and-play kit for launching tokenized invoices, property
          shares, and carbon credits on Stellar — with KYC and transfer compliance
          enforced at the protocol level, not bolted on after the fact.
        </p>
        <div style={styles.heroActions}>
          <Link to="/invoices" className="btn">
            Launch an asset <Icon.arrow size={16} style={{ display: "inline", verticalAlign: "-3px", marginLeft: 4 }} />
          </Link>
          <Link to="/kyc" className="btn btn-ghost">
            Open KYC registry
          </Link>
        </div>
      </section>

      {/* Stats */}
      <section style={styles.stats}>
        {STATS.map((s) => (
          <div key={s.label} className="card" style={styles.stat}>
            <div style={styles.statValue} className="text-gradient">
              {s.value}
            </div>
            <div style={{ fontSize: "0.85rem", fontWeight: 600 }}>{s.label}</div>
            <div className="muted" style={{ fontSize: "0.78rem" }}>
              {s.sub}
            </div>
          </div>
        ))}
      </section>

      {/* Token Info */}
      {CONTRACT_IDS.rwaToken && (
        <section className="card" style={{ marginBottom: "2.75rem" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "0.7rem", marginBottom: "1.25rem" }}>
            <div style={styles.complianceIcon}>
              <Icon.invoice size={22} />
            </div>
            <div>
              <h2 style={{ fontSize: "1.05rem", fontWeight: 700 }}>Token Info</h2>
              <p className="muted" style={{ fontSize: "0.85rem", marginTop: "0.2rem" }}>
                On-chain compliance configuration for the deployed RWA token
              </p>
            </div>
          </div>

          {tokenInfoLoading && (
            <p className="muted" style={{ fontSize: "0.9rem" }}>Loading token info…</p>
          )}

          {tokenInfoError && !tokenInfoLoading && (
            <p style={{ fontSize: "0.875rem", color: "var(--error, #f87171)" }}>{tokenInfoError}</p>
          )}

          {tokenInfo && !tokenInfoLoading && (
            <div style={styles.tokenGrid}>
              <TokenInfoRow label="Asset Type" value={tokenInfo.assetType || "—"} />
              <TokenInfoRow label="KYC Registry" value={tokenInfo.kycRegistry || "—"} mono />
              <TokenInfoRow label="Compliance Engine" value={tokenInfo.complianceEngine || "—"} mono />
              <TokenInfoRow label="Legal Entity" value={tokenInfo.legalEntity || "—"} />
              <TokenInfoRow label="Governing Law" value={tokenInfo.governingLaw || "—"} />
              <TokenInfoRow label="ISIN" value={tokenInfo.isin || "—"} mono />
            </div>
          )}
        </section>
      )}

      {/* Asset modules */}
      <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", margin: "0.5rem 0 1.1rem" }}>
        <h2 style={{ fontSize: "1.2rem", fontWeight: 700 }}>Asset modules</h2>
        <span className="muted" style={{ fontSize: "0.85rem" }}>
          Pick a workflow to begin
        </span>
      </div>
      <div style={styles.grid}>
        {CARDS.map((c) => (
          <Link key={c.href} to={c.href} style={{ textDecoration: "none" }}>
            <div className="card card-interactive" style={styles.card}>
              <div style={{ ...styles.iconTile, background: c.gradient }}>{c.icon}</div>
              <h3 style={{ fontSize: "1.05rem", fontWeight: 700, marginTop: "1rem" }}>{c.title}</h3>
              <p className="muted" style={{ fontSize: "0.875rem", marginTop: "0.4rem" }}>
                {c.description}
              </p>
              <span style={styles.cardLink}>
                Open <Icon.arrow size={14} style={{ display: "inline", verticalAlign: "-2px" }} />
              </span>
            </div>
          </Link>
        ))}
      </div>

      {/* Compliance strip */}
      <section className="card" style={styles.compliance}>
        <div style={{ display: "flex", alignItems: "center", gap: "0.7rem" }}>
          <div style={styles.complianceIcon}>
            <Icon.shield size={22} />
          </div>
          <div>
            <h3 style={{ fontSize: "1.05rem", fontWeight: 700 }}>Compliance is the foundation</h3>
            <p className="muted" style={{ fontSize: "0.875rem", marginTop: "0.2rem" }}>
              Every transfer clears two cross-contract checks before any balance moves.
            </p>
          </div>
        </div>
        <div style={styles.steps}>
          <div style={styles.step}>
            <span className="badge badge-accent">1</span>
            <div>
              <div style={{ fontWeight: 600, fontSize: "0.9rem" }}>KYC Registry</div>
              <div className="muted" style={{ fontSize: "0.8rem" }}>
                Sender &amp; receiver hold active, non-expired approval.
              </div>
            </div>
          </div>
          <div style={styles.step}>
            <span className="badge badge-accent">2</span>
            <div>
              <div style={{ fontWeight: 600, fontSize: "0.9rem" }}>Compliance Engine</div>
              <div className="muted" style={{ fontSize: "0.8rem" }}>
                Limits, blocklist, holding periods &amp; pause are enforced.
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  hero: { padding: "1.5rem 0 2.75rem", maxWidth: 760 },
  heroTitle: { fontSize: "3rem", fontWeight: 800, marginTop: "1rem", lineHeight: 1.08 },
  heroSub: { marginTop: "1.1rem", fontSize: "1.05rem", maxWidth: 620 },
  heroActions: { display: "flex", gap: "0.75rem", marginTop: "1.75rem", flexWrap: "wrap" },
  stats: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
    gap: "1rem",
    marginBottom: "2.75rem",
  },
  stat: { padding: "1.25rem 1.35rem" },
  statValue: { fontSize: "2.1rem", fontWeight: 800, lineHeight: 1, marginBottom: "0.5rem" },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))",
    gap: "1.25rem",
  },
  card: { display: "flex", flexDirection: "column", height: "100%" },
  iconTile: {
    width: 46,
    height: 46,
    borderRadius: 13,
    display: "grid",
    placeItems: "center",
    color: "#fff",
    boxShadow: "0 8px 22px rgba(0,0,0,0.35)",
  },
  cardLink: {
    marginTop: "1.1rem",
    display: "inline-flex",
    alignItems: "center",
    gap: "0.35rem",
    fontSize: "0.85rem",
    fontWeight: 600,
    color: "var(--accent-2)",
  },
  tokenGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
    gap: "1rem",
  },
  compliance: { marginTop: "2.75rem" },
  complianceIcon: {
    display: "grid",
    placeItems: "center",
    width: 46,
    height: 46,
    borderRadius: 13,
    background: "var(--accent-soft)",
    color: "var(--accent-2)",
    flexShrink: 0,
  },
  steps: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "1rem",
    marginTop: "1.5rem",
  },
  step: {
    display: "flex",
    gap: "0.7rem",
    alignItems: "flex-start",
    padding: "1rem",
    borderRadius: 12,
    background: "var(--surface-2)",
    border: "1px solid var(--border)",
  },
};
