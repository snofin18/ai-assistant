import type { CostPeriod } from "./costModel.js";

/**
 * All user-visible cost panel strings are injected here.
 */
export interface CostPanelCopy {
  heading: string;
  totalCostLabel: string;
  callCountLabel: string;
  inputTokensLabel: string;
  cachedTokensLabel: string;
  outputTokensLabel: string;
  latencyLabel: string;
  byModelHeading: string;
  byAppHeading: string;
  modelColumnLabel: string;
  appColumnLabel: string;
  validationErrorLabel: string;
  periodLabels: Record<CostPeriod, string>;
  formatMicroUsd: (microUsd: number) => string;
  formatTokens: (tokens: number) => string;
  formatLatency: (latencyMs: number) => string;
}
