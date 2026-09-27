import type {
  TimelineReversibilityLevel,
  TimelineStepStatus,
  VerificationStatus,
} from "./timelineModel.js";

/**
 * All user-visible timeline strings are injected here.
 */
export interface TimelineCopy {
  heading: string;
  empty: string;
  stepStatusLabels: Record<TimelineStepStatus, string>;
  verificationLabels: Record<VerificationStatus, string>;
  reversibilityLabels: Record<TimelineReversibilityLevel, string>;
  preFingerprintLabel: string;
  postFingerprintLabel: string;
  evidenceLabel: string;
  evidenceKindLabels: Record<"tree_snapshot" | "screenshot" | "log", string>;
  evidenceMessages: Record<string, string>;
  durationLabel: (durationMs: number) => string;
  costLabel: (usd: number) => string;
  tokensLabel: (tokensIn: number, tokensOut: number) => string;
  undoLabel: string;
  replayLabel: string;
  selectedLabel: string;
  validationErrorLabel: string;
  operationMessages: Record<string, string>;
}
