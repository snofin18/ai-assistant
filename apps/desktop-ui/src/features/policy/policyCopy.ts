import type { EgressLevel } from "./egressPolicy.js";

/**
 * All user-visible egress policy strings are injected here.
 */
export interface EgressPolicyCopy {
  panelHeading: string;
  defaultLevelLabel: string;
  appOverrideLabel: string;
  appIdLabel: string;
  clearOverrideLabel: string;
  upgradeWarningLabel: string;
  confirmUpgradeLabel: string;
  cancelUpgradeLabel: string;
  localModelUnavailableLabel: string;
  validationErrorLabel: string;
  statusLabel: string;
  invalidPolicyLabel: string;
  levelLabels: Record<EgressLevel, string>;
  levelDescriptions: Record<EgressLevel, string>;
  errorMessages: Record<string, string>;
}
