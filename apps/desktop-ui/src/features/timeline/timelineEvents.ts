/**
 * Projects validated Core events onto the execution timeline.
 *
 * An unknown Core step status is never mapped onto a success-like timeline
 * state: the event is dropped and the caller keeps showing the last verified
 * state, so a future status cannot be mistaken for "succeeded".
 */

import type { UiEvent } from "../../ipc/contract.js";
import type {
  TimelineStepModel,
  TimelineStepStatus,
  VerificationStatus,
} from "./timelineModel.js";

const timelineStatusByCoreStatus: Readonly<Record<string, TimelineStepStatus>> = {
  committed: "succeeded",
  failed: "failed",
  policy_denied: "failed",
  rolled_back: "undone",
};

export function timelineStatusFromCoreStatus(status: string): TimelineStepStatus | null {
  return timelineStatusByCoreStatus[status] ?? null;
}

export function verificationFromCoreStatus(status: string): VerificationStatus {
  if (status === "committed") {
    return "passed";
  }
  if (status === "failed" || status === "policy_denied") {
    return "failed";
  }
  return "not_applicable";
}

/**
 * Returns an updated step when the event targets it, otherwise `null`.
 */
export function applyUiEventToStep(
  step: TimelineStepModel,
  event: UiEvent
): TimelineStepModel | null {
  if (event.kind !== "step_state_changed" || event.step_id !== step.stepId) {
    return null;
  }
  const status = timelineStatusFromCoreStatus(event.status);
  if (status === null) {
    return null;
  }
  return {
    ...step,
    status,
    verification: verificationFromCoreStatus(event.status),
    postFingerprint: event.post_fingerprint ?? step.postFingerprint,
  };
}

export function applyUiEventToTimeline(
  steps: readonly TimelineStepModel[],
  event: UiEvent
): TimelineStepModel[] {
  return steps.map((step) => applyUiEventToStep(step, event) ?? step);
}
