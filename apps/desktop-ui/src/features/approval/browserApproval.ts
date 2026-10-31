/**
 * Maps an already strict-parsed browser form approval event to the approval card.
 *
 * The mapper never calls IPC or treats the displayed event as an authorization.
 */

import type { UiEvent } from "../../ipc/contract.js";
import type { ApprovalCardModel } from "./approvalModel.js";

export type StepApprovalRequestedEvent = Extract<UiEvent, { kind: "step_approval_requested" }>;

export function toApprovalCardModel(
  event: StepApprovalRequestedEvent,
  nowMs = Date.now(),
): ApprovalCardModel {
  return {
    requestId: event.request_id,
    action: event.action,
    targetLabel: event.target,
    summary: event.summary,
    impact: event.impact,
    diff: {
      kind: "fields",
      entries: event.diff.entries.map((entry) => ({
        field: entry.field,
        before: entry.before,
        after: entry.after,
      })),
    },
    instructionOrigin: event.instruction_attribution.origin,
    toolSelectionReason: event.action,
    riskLevel: event.risk,
    reversibility: "L3Irreversible",
    undo: {
      isAvailable: false,
      methodKey: "approval.undo.none",
      anchorId: null,
      disabledReasonKey: "approval.undo.irreversible",
    },
    scopeOptions: [...event.scope_options],
    evidence: [
      {
        id: `form-state:${event.request_id}`,
        kind: "form_state",
        labelKey: "approval.evidence.form_state",
        href: null,
      },
    ],
    isExpired: nowMs >= event.expires_at_ms,
  };
}
