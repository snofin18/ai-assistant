/**
 * Maps an approval-card decision onto the UI → Core command contract.
 *
 * The mapping never authorizes anything: it only re-expresses the user's
 * decision in the shape Core validates. The result still crosses
 * `sendUiCommand`'s contract check before it is sent, so a malformed decision
 * surfaces as an explicit error instead of a silently dropped approval.
 */

import type { UiCommand } from "../../ipc/contract.js";
import type { ApprovalDecisionEvent } from "./approvalModel.js";

export function toApprovalCommand(decision: ApprovalDecisionEvent): UiCommand {
  if (decision.kind === "approved") {
    return {
      kind: "approve_request",
      request_id: decision.requestId,
      scope: decision.scope,
    };
  }
  return {
    kind: "deny_request",
    request_id: decision.requestId,
    reason: decision.reason,
  };
}
