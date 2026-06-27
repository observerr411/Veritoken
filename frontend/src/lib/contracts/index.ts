/**
 * Singleton contract clients.
 *
 * Import pattern (recommended):
 *   import { contracts } from "../lib/contracts";
 *   const rules = await contracts.compliance.getRules();
 *
 * Each client is initialised once with the contract ID from the environment
 * and the shared RPC server instance. Re-exports all client classes so that
 * consumers can also import them directly for typing or testing purposes.
 */

import { server, CONTRACT_IDS } from "../stellar";
import { KycRegistryClient } from "./kycRegistry";
import { ComplianceEngineClient } from "./complianceEngine";
import { InvoiceTokenClient } from "./invoiceToken";
import { PropertyTokenClient } from "./propertyToken";
import { CarbonTokenClient } from "./carbonToken";
import { RwaTokenClient } from "./rwaToken";

// Re-export client classes for typing and isolated instantiation in tests.
export { KycRegistryClient } from "./kycRegistry";
export { ComplianceEngineClient } from "./complianceEngine";
export { InvoiceTokenClient } from "./invoiceToken";
export { PropertyTokenClient } from "./propertyToken";
export { CarbonTokenClient } from "./carbonToken";
export { RwaTokenClient } from "./rwaToken";
export type { SignTx } from "./base";

// ── Singleton ─────────────────────────────────────────────────────────────────

export const contracts = {
  /** KYC registry — verifier management and approval status. */
  kyc: new KycRegistryClient(CONTRACT_IDS.kycRegistry, server),

  /** Compliance engine — rules, blocklist, pause, and transfer validation. */
  compliance: new ComplianceEngineClient(CONTRACT_IDS.complianceEngine, server),

  /** Invoice token — accounts-receivable tokenization. */
  invoice: new InvoiceTokenClient(CONTRACT_IDS.invoiceToken, server),

  /** Property token — fractional real estate ownership. */
  property: new PropertyTokenClient(CONTRACT_IDS.propertyToken, server),

  /** Carbon credit token — verified tonne retirement. */
  carbon: new CarbonTokenClient(CONTRACT_IDS.carbonToken, server),

  /** RWA base token — compliance metadata, asset type, registry/engine addresses. */
  rwa: new RwaTokenClient(CONTRACT_IDS.rwaToken, server),
} as const;
