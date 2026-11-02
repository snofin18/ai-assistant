import { useEffect, useState } from "react";

import { ApprovalDiffView } from "./features/approval/ApprovalDiffView.js";
import { ApprovalCard, toApprovalCardModel, toApprovalCommand } from "./features/approval/index.js";
import type {
  ApprovalCardCopy,
  ApprovalDecisionEvent,
  StepApprovalRequestedEvent,
} from "./features/approval/index.js";
import { IntentLauncher } from "./features/intent/IntentLauncher.js";
import { connectUiEventStream, HostIpcError, sendUiCommand } from "./ipc/client.js";
import type { UiEvent } from "./ipc/contract.js";

interface AppProps {
  connectEvents?: typeof connectUiEventStream;
  sendCommand?: typeof sendUiCommand;
  now?: () => number;
}

export function App({
  connectEvents = connectUiEventStream,
  sendCommand = sendUiCommand,
  now = Date.now,
}: AppProps) {
  const [approval, setApproval] = useState<StepApprovalRequestedEvent | null>(null);
  const [bridgeError, setBridgeError] = useState<string | null>(null);
  const [operationNotice, setOperationNotice] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    let isActive = true;
    let unsubscribe: (() => void) | null = null;
    void connectEvents({
      onEvent: (event: UiEvent) => {
        if (isActive && event.kind === "step_approval_requested") {
          setApproval(event);
          setBridgeError(null);
        }
      },
      onFailure: (error: HostIpcError) => {
        if (isActive) {
          setBridgeError(`${error.code}: ${error.details.join("; ")}`);
        }
      },
    })
      .then((cleanup) => {
        if (isActive) {
          unsubscribe = cleanup;
        } else {
          cleanup();
        }
      })
      .catch((error: unknown) => {
        if (isActive) {
          setBridgeError(
            error instanceof HostIpcError ? error.message : "ui_event_stream_unavailable",
          );
        }
      });
    return () => {
      isActive = false;
      unsubscribe?.();
    };
  }, [connectEvents]);

  async function submitDecision(decision: ApprovalDecisionEvent) {
    setIsSubmitting(true);
    setOperationNotice(null);
    try {
      await sendCommand(toApprovalCommand(decision));
      setApproval(null);
      setOperationNotice(
        decision.kind === "approved" ? "Approval sent to Core." : "Denial sent to Core.",
      );
    } catch (error) {
      setOperationNotice(
        error instanceof HostIpcError ? error.message : "Approval command failed.",
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  const approvalModel = approval === null ? null : toApprovalCardModel(approval, now());
  const approvalCardModel =
    approvalModel === null
      ? null
      : {
          ...approvalModel,
          diff: {
            kind: "irreversible" as const,
            warningKey: "approval.warning.point_of_no_return",
          },
        };
  const fieldDiff = approvalModel === null ? null : approvalModel.diff;

  return (
    <main className="min-h-screen bg-slate-950 p-8 text-slate-100">
      <section className="mx-auto max-w-2xl border border-slate-700 p-6">
        <h1 className="text-xl font-semibold">Assistant Host</h1>
        <p className="mt-2 text-sm text-slate-300">
          Describe a task; the assistant plans and executes it under policy.
        </p>
      </section>
      <div className="mx-auto mt-6 max-w-2xl">
        <IntentLauncher />
      </div>
      {bridgeError === null ? null : (
        <p
          className="mx-auto mt-4 max-w-2xl border border-rose-500 p-3 text-sm text-rose-200"
          role="alert"
        >
          Event stream unavailable: {bridgeError}
        </p>
      )}
      {operationNotice === null ? null : (
        <p
          className="mx-auto mt-4 max-w-2xl border border-slate-600 p-3 text-sm text-slate-200"
          role="status"
        >
          {operationNotice}
        </p>
      )}
      {approval === null || approvalCardModel === null || fieldDiff === null ? null : (
        <div className="mx-auto mt-6 max-w-3xl">
          <ApprovalCard
            copy={defaultApprovalCopy}
            model={approvalCardModel}
            onDecision={(decision) => {
              if (!isSubmitting) {
                void submitDecision(decision);
              }
            }}
            onDryRun={() => setOperationNotice("Dry-run is not available in this bridge slice.")}
            onRequestModification={() =>
              setOperationNotice("Modify the task goal to change these parameters.")
            }
          />
          <section className="mt-4 border border-slate-700 p-4">
            <h2 className="mb-2 text-sm font-medium">Proposed field changes</h2>
            <ApprovalDiffView copy={defaultApprovalCopy} diff={fieldDiff} />
          </section>
        </div>
      )}
    </main>
  );
}

const defaultApprovalCopy: ApprovalCardCopy = {
  cardHeading: "Human approval required",
  actionLabel: "Action",
  targetLabel: "Target",
  summaryLabel: "Summary",
  impactLabel: "Impact",
  diffLabel: "Proposed field changes",
  noDiff: "No field changes",
  irreversibleWarning: "This action cannot be undone.",
  diffField: "Field",
  diffBefore: "Before",
  diffAfter: "After",
  screenshotEvidence: "Screenshot",
  warningMessages: { "approval.warning.point_of_no_return": "Point of no return" },
  notApplicable: "Not applicable",
  formatBytes: (bytes) => `${bytes} bytes`,
  originLabel: "Instruction source",
  originLabels: {
    user_request: "User request",
    plan_derived: "Plan",
    app_content: "Application content",
    tool_suggestion: "Tool suggestion",
  },
  appContentWarning: "Application content is untrusted.",
  appContentOverrideLabel: "I explicitly reviewed the application content",
  irreversibleConfirmationLabel: "Type once to confirm",
  irreversibleConfirmationPhrase: "once",
  toolReasonLabel: "Why this tool",
  riskLabel: "Risk",
  riskLabels: { low: "Low", medium: "Medium", high: "High", critical: "Critical" },
  reversibilityLabel: "Reversibility",
  reversibilityLabels: {
    L0UndoStack: "Application undo",
    L1ContentSnapshot: "Content snapshot",
    L2ShadowCopy: "Shadow copy",
    L3Irreversible: "Irreversible",
  },
  undoLabel: "Undo",
  undoMethodMessages: {
    "approval.undo.none": "No undo path",
    "approval.undo.content_snapshot": "Restore content snapshot",
  },
  authorizationScopeLabel: "Authorization",
  scopeLabels: {
    once: "Once",
    this_step_pattern: "This step pattern",
    this_task: "This task",
    this_app_session: "This app session",
    persistent: "Persistent",
  },
  evidenceLabel: "Evidence",
  evidenceKindLabels: {
    tree_snapshot: "Tree snapshot",
    screenshot: "Screenshot",
    log: "Log",
    form_state: "Form state",
  },
  evidenceMessages: {},
  expiredWarning: "This approval request has expired.",
  validationErrorLabel: "Approval data is invalid.",
  denialReasonLabel: "Reason for denial",
  denyLabel: "Deny",
  modifyParametersLabel: "Modify parameters",
  dryRunLabel: "Dry run",
  approveLabel: "Approve once",
  fileChangeLabels: { added: "Added", removed: "Removed", modified: "Modified" },
  errorMessages: {
    "approval.error.expired": "This approval request has expired.",
    "approval.error.scope_required": "Select an authorization scope.",
    "approval.error.high_risk_once_only": "High-risk actions allow only once.",
    "approval.error.irreversible_confirmation_required": "Type once to confirm.",
    "approval.error.denial_reason_required": "Enter a reason for denial.",
    "approval.error.app_content_override_required": "Review the application content first.",
    "approval.undo.irreversible": "This action cannot be undone.",
  },
};
