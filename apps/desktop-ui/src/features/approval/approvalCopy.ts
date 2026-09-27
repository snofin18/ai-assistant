import type {
  ApprovalRiskLevel,
  AuthorizationScope,
  InstructionOrigin,
  ReversibilityLevel,
} from "./approvalModel.js";

/**
 * All user-visible approval-card strings are injected here.
 */
export interface ApprovalCardCopy {
  cardHeading: string;
  actionLabel: string;
  targetLabel: string;
  summaryLabel: string;
  impactLabel: string;
  diffLabel: string;
  noDiff: string;
  irreversibleWarning: string;
  diffField: string;
  diffBefore: string;
  diffAfter: string;
  screenshotEvidence: string;
  warningMessages: Record<string, string>;
  notApplicable: string;
  formatBytes: (bytes: number) => string;
  originLabel: string;
  originLabels: Record<InstructionOrigin, string>;
  appContentWarning: string;
  appContentOverrideLabel: string;
  irreversibleConfirmationLabel: string;
  irreversibleConfirmationPhrase: string;
  toolReasonLabel: string;
  riskLabel: string;
  riskLabels: Record<ApprovalRiskLevel, string>;
  reversibilityLabel: string;
  reversibilityLabels: Record<ReversibilityLevel, string>;
  undoLabel: string;
  undoMethodMessages: Record<string, string>;
  authorizationScopeLabel: string;
  scopeLabels: Record<AuthorizationScope, string>;
  evidenceLabel: string;
  evidenceKindLabels: Record<"tree_snapshot" | "screenshot" | "log", string>;
  evidenceMessages: Record<string, string>;
  expiredWarning: string;
  validationErrorLabel: string;
  denialReasonLabel: string;
  denyLabel: string;
  modifyParametersLabel: string;
  dryRunLabel: string;
  approveLabel: string;
  fileChangeLabels: Record<"added" | "removed" | "modified", string>;
  errorMessages: Record<string, string>;
}
