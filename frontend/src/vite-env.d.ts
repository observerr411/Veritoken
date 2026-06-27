/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_STELLAR_NETWORK?: string;
  readonly VITE_KYC_REGISTRY_ID?: string;
  readonly VITE_COMPLIANCE_ENGINE_ID?: string;
  readonly VITE_INVOICE_TOKEN_ID?: string;
  readonly VITE_PROPERTY_TOKEN_ID?: string;
  readonly VITE_CARBON_TOKEN_ID?: string;
  readonly VITE_RWA_TOKEN_ID?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
