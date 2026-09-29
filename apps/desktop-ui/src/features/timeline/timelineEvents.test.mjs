import assert from "node:assert/strict";
import test from "node:test";

import {
  applyUiEventToStep,
  applyUiEventToTimeline,
  timelineStatusFromCoreStatus,
  verificationFromCoreStatus,
} from "./timelineEvents.ts";

function step(overrides = {}) {
  return {
    stepId: "s_1",
    action: "notepad.replace_text",
    targetLabel: "Quarterly report",
    status: "awaiting_approval",
    preFingerprint: null,
    postFingerprint: null,
    verification: "not_applicable",
    verificationReasonKey: null,
    evidence: [],
    durationMs: 0,
    cost: { tokensIn: 0, tokensOut: 0, usd: 0, latencyMs: 0 },
    reversibility: "L0UndoStack",
    undo: { isAvailable: false, methodKey: "undo.none", anchorId: null, disabledReasonKey: "undo.none" },
    canReplay: false,
    replayDisabledReasonKey: null,
    ...overrides,
  };
}

test("a committed step shows a real verification result", () => {
  const updated = applyUiEventToStep(step(), {
    kind: "step_state_changed",
    task_id: "t_1",
    step_id: "s_1",
    status: "committed",
    phase: null,
    post_fingerprint: "sha256:post",
  });
  assert.equal(updated?.status, "succeeded");
  assert.equal(updated?.verification, "passed");
  assert.equal(updated?.postFingerprint, "sha256:post");
});

test("a failed step is not shown as success", () => {
  const updated = applyUiEventToStep(step(), {
    kind: "step_state_changed",
    task_id: "t_1",
    step_id: "s_1",
    status: "failed",
    phase: null,
    post_fingerprint: null,
  });
  assert.equal(updated?.status, "failed");
  assert.equal(updated?.verification, "failed");
});

test("an unknown Core status is dropped rather than guessed", () => {
  assert.equal(timelineStatusFromCoreStatus("future_state"), null);
  const updated = applyUiEventToStep(step(), {
    kind: "step_state_changed",
    task_id: "t_1",
    step_id: "s_1",
    status: "future_state",
    phase: null,
    post_fingerprint: null,
  });
  assert.equal(updated, null);
  assert.equal(verificationFromCoreStatus("future_state"), "not_applicable");
});

test("events for other steps leave the timeline unchanged", () => {
  const steps = [step()];
  const next = applyUiEventToTimeline(steps, {
    kind: "step_state_changed",
    task_id: "t_1",
    step_id: "s_other",
    status: "committed",
    phase: null,
    post_fingerprint: "sha256:other",
  });
  assert.deepEqual(next, steps);
});

test("task-level events do not rewrite step rows", () => {
  const steps = [step()];
  const next = applyUiEventToTimeline(steps, {
    kind: "task_state_changed",
    task_id: "t_1",
    status: "running",
    hold_reason: null,
  });
  assert.deepEqual(next, steps);
});
