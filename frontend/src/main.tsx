import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import "./index.css";

// ── Environment variable validation ──────────────────────────────────────────

const REQUIRED_ENV_VARS: { key: keyof ImportMetaEnv; label: string }[] = [
  { key: "VITE_KYC_REGISTRY_ID",      label: "KYC Registry contract ID" },
  { key: "VITE_COMPLIANCE_ENGINE_ID", label: "Compliance Engine contract ID" },
  { key: "VITE_INVOICE_TOKEN_ID",     label: "Invoice Token contract ID" },
  { key: "VITE_PROPERTY_TOKEN_ID",    label: "Property Token contract ID" },
  { key: "VITE_CARBON_TOKEN_ID",      label: "Carbon Token contract ID" },
];

const missingVars = REQUIRED_ENV_VARS.filter(({ key }) => !import.meta.env[key]);

// ── Config error screen ───────────────────────────────────────────────────────

function ConfigError({ missing }: { missing: typeof REQUIRED_ENV_VARS }) {
  return (
    <div style={{
      minHeight: "100vh",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      background: "var(--bg, #0f1117)",
      padding: "1.5rem",
      fontFamily: "system-ui, sans-serif",
    }}>
      <div style={{
        background: "var(--surface, #1a1d27)",
        border: "1px solid var(--border, #2a2d3a)",
        borderRadius: 16,
        padding: "2rem 2.25rem",
        maxWidth: 480,
        width: "100%",
      }}>
        <h1 style={{ fontSize: "1.25rem", fontWeight: 700, marginBottom: "0.5rem" }}>
          Configuration incomplete
        </h1>
        <p style={{ color: "var(--muted, #8b8fa8)", fontSize: "0.9rem", marginBottom: "1.25rem" }}>
          The following environment variables are missing or empty:
        </p>
        <ul style={{ listStyle: "none", padding: 0, margin: "0 0 1.5rem" }}>
          {missing.map(({ key, label }) => (
            <li key={key} style={{
              display: "flex",
              flexDirection: "column",
              gap: "0.15rem",
              padding: "0.6rem 0.8rem",
              marginBottom: "0.5rem",
              borderRadius: 8,
              background: "var(--surface-2, #22253a)",
              border: "1px solid var(--border, #2a2d3a)",
            }}>
              <code style={{ fontSize: "0.82rem", color: "var(--accent-2, #818cf8)" }}>{key}</code>
              <span style={{ fontSize: "0.8rem", color: "var(--muted, #8b8fa8)" }}>{label}</span>
            </li>
          ))}
        </ul>
        <p style={{ color: "var(--muted, #8b8fa8)", fontSize: "0.85rem", marginBottom: "1.25rem" }}>
          Copy <code style={{ fontSize: "0.82rem" }}>frontend/.env.example</code> to{" "}
          <code style={{ fontSize: "0.82rem" }}>frontend/.env</code> and fill in the contract IDs
          from your deployment. Then reload this page.
        </p>
        <button
          onClick={() => window.location.reload()}
          style={{
            width: "100%",
            padding: "0.65rem 1rem",
            borderRadius: 8,
            border: "none",
            background: "var(--accent, #6366f1)",
            color: "#fff",
            fontWeight: 600,
            fontSize: "0.9rem",
            cursor: "pointer",
          }}
        >
          Reload
        </button>
      </div>
    </div>
  );
}

// ── Boot ──────────────────────────────────────────────────────────────────────

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 30_000 },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {missingVars.length > 0 ? (
      <ConfigError missing={missingVars} />
    ) : (
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </QueryClientProvider>
    )}
  </React.StrictMode>
);
